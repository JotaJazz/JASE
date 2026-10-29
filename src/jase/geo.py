"""País de un servidor por su IPv4: base DB-IP «IP to Country Lite» (CC BY 4.0).

La base es un CSV `ip_inicio,ip_fin,país` con rangos IPv4 e IPv6 mezclados
(~360 mil filas IPv4, ~30 MB). Solo se guardan los IPv4, en dos `array('I')`
ordenados más un índice compacto al código de país; la búsqueda es `bisect`.
Cargar y descargar son operaciones bloqueantes: llamarlas desde un hilo.
"""
from __future__ import annotations

import contextlib
import gzip
import io
import logging
import os
import socket
import tempfile
from array import array
from bisect import bisect_right
from collections.abc import Callable
from datetime import date
from pathlib import Path

from jase import i18n
from jase.i18n import tr
from jase.locales.countries_de import COUNTRY_NAMES_DE
from jase.locales.countries_en import COUNTRY_NAMES_EN
from jase.locales.countries_fr import COUNTRY_NAMES_FR
from jase.locales.countries_it import COUNTRY_NAMES_IT
from jase.locales.countries_pt import COUNTRY_NAMES_PT
from jase.net import default_fetch

log = logging.getLogger(__name__)

DB_URL = "https://download.db-ip.com/free/dbip-country-lite-{month}.csv.gz"
DB_FILENAME = "dbip-country-lite.csv"
# El .csv.gz pesa ~4,5 MB; descomprimido ~30 MB. Márgenes amplios pero acotados.
MAX_GZ_BYTES = 32 * 1024 * 1024
MAX_CSV_BYTES = 256 * 1024 * 1024
DOWNLOAD_DEADLINE = 180  # segundos para toda la descarga
UNKNOWN_CODES = {"zz"}  # DB-IP marca así los rangos reservados / sin asignar

Fetch = Callable[[str], "bytes | None"]


def _download_fetch(url: str) -> bytes | None:
    return default_fetch(url, max_bytes=MAX_GZ_BYTES, deadline=DOWNLOAD_DEADLINE)


def _ipv4_int(text: str) -> int | None:
    """Entero de una IPv4 en notación decimal con puntos estricta, o None."""
    parts = text.split(".")
    if len(parts) != 4:
        return None
    value = 0
    for part in parts:
        if not (part.isdigit() and part.isascii() and len(part) <= 3):
            return None
        octet = int(part)
        if octet > 255:
            return None
        value = (value << 8) | octet
    return value


def _fast_ipv4_int(text: str) -> int | None:
    """Como `_ipv4_int` pero en C (inet_aton); para las ~360 mil filas de la base."""
    if text.count(".") != 3:
        return None
    try:
        return int.from_bytes(socket.inet_aton(text), "big")
    except (OSError, ValueError):
        return None


class CountryDB:
    """Rangos IPv4 → código ISO alfa-2 (minúsculas)."""

    def __init__(self, starts: array, ends: array, code_index: array, codes: list[str]) -> None:
        self._starts = starts
        self._ends = ends
        self._code_index = code_index
        self._codes = codes

    def __len__(self) -> int:
        return len(self._starts)

    @classmethod
    def from_csv(cls, path: Path) -> CountryDB:
        """Lee el CSV de DB-IP. Filas IPv6, mal formadas o desconocidas (ZZ) se omiten.

        OSError si el archivo no se puede leer.
        """
        starts, ends, code_index = array("I"), array("I"), array("H")
        codes: list[str] = []
        positions: dict[str, int] = {}
        ordered = True
        last_start = -1
        with open(path, encoding="ascii", errors="replace", newline="") as fh:
            for line in fh:
                if ":" in line:  # IPv6
                    continue
                fields = line.strip().split(",")
                if len(fields) != 3:
                    continue
                start, end = _fast_ipv4_int(fields[0]), _fast_ipv4_int(fields[1])
                code = fields[2].strip().strip('"').lower()
                if (start is None or end is None or end < start or len(code) != 2
                        or not code.isalpha() or not code.isascii() or code in UNKNOWN_CODES):
                    continue
                pos = positions.get(code)
                if pos is None:
                    pos = positions[code] = len(codes)
                    codes.append(code)
                if start < last_start:
                    ordered = False
                last_start = start
                starts.append(start)
                ends.append(end)
                code_index.append(pos)
        if not ordered:
            rows = sorted(zip(starts, ends, code_index, strict=True))
            starts = array("I", (r[0] for r in rows))
            ends = array("I", (r[1] for r in rows))
            code_index = array("H", (r[2] for r in rows))
        return cls(starts, ends, code_index, codes)

    def lookup(self, ip: str | None) -> str | None:
        if not isinstance(ip, str):
            return None
        value = _ipv4_int(ip)
        if value is None:
            return None
        i = bisect_right(self._starts, value) - 1
        if i < 0 or value > self._ends[i]:
            return None
        return self._codes[self._code_index[i]]


# ------------------------------------------------------------------ descarga
def _months(today: date) -> list[str]:
    previous = date(today.year - 1, 12, 1) if today.month == 1 else date(
        today.year, today.month - 1, 1)
    return [f"{today:%Y-%m}", f"{previous:%Y-%m}"]


def _looks_like_db(path: Path) -> bool:
    """Primera fila con forma `ipv4,ipv4,cc` (descarta páginas de error comprimidas)."""
    try:
        with open(path, encoding="ascii", errors="replace") as fh:
            fields = fh.readline().strip().split(",")
    except OSError:
        return False
    return (len(fields) == 3 and _ipv4_int(fields[0]) is not None
            and _ipv4_int(fields[1]) is not None and len(fields[2]) == 2)


def _install(data: bytes, dest: Path) -> bool:
    """Descomprime `data` en `dest` de forma atómica; False si no es una base válida."""
    tmp = None
    try:
        dest.parent.mkdir(parents=True, exist_ok=True)
        fd, tmp = tempfile.mkstemp(prefix=f".{dest.name}.", suffix=".tmp", dir=dest.parent)
        with os.fdopen(fd, "wb") as out, gzip.GzipFile(fileobj=io.BytesIO(data)) as gz:
            written = 0
            while chunk := gz.read(1024 * 1024):
                written += len(chunk)
                if written > MAX_CSV_BYTES:
                    raise ValueError("base de países demasiado grande")
                out.write(chunk)
        if not _looks_like_db(Path(tmp)):
            raise ValueError("no parece una base DB-IP")
        os.replace(tmp, dest)
        return True
    except Exception as exc:  # gzip inválido, disco lleno, permisos…
        log.debug("No se pudo instalar la base de países en %s: %s", dest, exc)
        if tmp is not None:
            with contextlib.suppress(OSError):
                os.unlink(tmp)
        return False


def download_db(dest: Path, today: date, fetch: Fetch | None = None) -> bool:
    """Descarga la base del mes actual (o, si falla, la del anterior) a `dest`.

    El archivo existente solo se reemplaza si la descarga es válida.
    """
    fetch = fetch or _download_fetch
    dest = Path(dest)
    for month in _months(today):
        url = DB_URL.format(month=month)
        try:
            data = fetch(url)
        except Exception as exc:
            log.debug("Descarga fallida %s: %s", url, exc)
            data = None
        if data and _install(data, dest):
            return True
    return False


class CountrySource:
    """Obtiene la base: del archivo en caché o, si falta (o se pide), descargándola.

    `load` es bloqueante y nunca lanza: devuelve (base, None) o (None, mensaje).
    """

    def __init__(self, fetch: Fetch | None = None,
                 today: Callable[[], date] = date.today) -> None:
        self._fetch = fetch
        self._today = today

    def load(self, path: Path, refresh: bool = False) -> tuple[CountryDB | None, str | None]:
        path = Path(path)
        try:
            if (refresh or not path.is_file()) and not download_db(
                    path, self._today(), fetch=self._fetch):
                return None, tr("No se pudo descargar la base de países")
            return CountryDB.from_csv(path), None
        except Exception as exc:  # OSError, MemoryError…
            log.debug("No se pudo leer la base de países %s: %s", path, exc)
            return None, tr("No se pudo leer la base de países")


# -------------------------------------------------------- nombres en español
# QLocale no sirve: territoryToString() da nombres en inglés y nativeTerritoryName()
# solo traduce si existe un locale español para ese territorio (DE → "España").
# Tabla propia con los nombres de CLDR (es) para los códigos de flag-icons.
_NAMES_ES = """\
ad Andorra|ae Emiratos Árabes Unidos|af Afganistán|ag Antigua y Barbuda|ai Anguila
al Albania|am Armenia|ao Angola|aq Antártida|ar Argentina|as Samoa Americana|at Austria
au Australia|aw Aruba|ax Islas Aland|az Azerbaiyán|ba Bosnia y Herzegovina|bb Barbados
bd Bangladés|be Bélgica|bf Burkina Faso|bg Bulgaria|bh Baréin|bi Burundi|bj Benín
bl San Bartolomé|bm Bermudas|bn Brunéi|bo Bolivia|bq Caribe neerlandés|br Brasil
bs Bahamas|bt Bután|bv Isla Bouvet|bw Botsuana|by Bielorrusia|bz Belice|ca Canadá
cc Islas Cocos|cd República Democrática del Congo|cf República Centroafricana
cg Congo|ch Suiza|ci Côte d’Ivoire|ck Islas Cook|cl Chile|cm Camerún|cn China
co Colombia|cp Isla Clipperton|cr Costa Rica|cu Cuba|cv Cabo Verde|cw Curazao
cx Isla de Navidad|cy Chipre|cz Chequia|de Alemania|dg Diego García|dj Yibuti
dk Dinamarca|dm Dominica|do República Dominicana|dz Argelia|ec Ecuador|ee Estonia
eg Egipto|eh Sáhara Occidental|er Eritrea|es España|et Etiopía|eu Unión Europea
fi Finlandia|fj Fiyi|fk Islas Malvinas|fm Micronesia|fo Islas Feroe|fr Francia
ga Gabón|gb Reino Unido|gd Granada|ge Georgia|gf Guayana Francesa|gg Guernesey
gh Ghana|gi Gibraltar|gl Groenlandia|gm Gambia|gn Guinea|gp Guadalupe
gq Guinea Ecuatorial|gr Grecia|gs Islas Georgia del Sur y Sandwich del Sur
gt Guatemala|gu Guam|gw Guinea-Bisáu|gy Guyana|hk Hong Kong|hm Islas Heard y McDonald
hn Honduras|hr Croacia|ht Haití|hu Hungría|ic Canarias|id Indonesia|ie Irlanda
il Israel|im Isla de Man|in India|io Territorio Británico del Océano Índico|iq Irak
ir Irán|is Islandia|it Italia|je Jersey|jm Jamaica|jo Jordania|jp Japón|ke Kenia
kg Kirguistán|kh Camboya|ki Kiribati|km Comoras|kn San Cristóbal y Nieves
kp Corea del Norte|kr Corea del Sur|kw Kuwait|ky Islas Caimán|kz Kazajistán|la Laos
lb Líbano|lc Santa Lucía|li Liechtenstein|lk Sri Lanka|lr Liberia|ls Lesoto
lt Lituania|lu Luxemburgo|lv Letonia|ly Libia|ma Marruecos|mc Mónaco|md Moldavia
me Montenegro|mf San Martín|mg Madagascar|mh Islas Marshall|mk Macedonia del Norte
ml Mali|mm Myanmar (Birmania)|mn Mongolia|mo Macao|mp Islas Marianas del Norte
mq Martinica|mr Mauritania|ms Montserrat|mt Malta|mu Mauricio|mv Maldivas|mw Malaui
mx México|my Malasia|mz Mozambique|na Namibia|nc Nueva Caledonia|ne Níger
nf Isla Norfolk|ng Nigeria|ni Nicaragua|nl Países Bajos|no Noruega|np Nepal|nr Nauru
nu Niue|nz Nueva Zelanda|om Omán|pa Panamá|pc Comunidad del Pacífico|pe Perú
pf Polinesia Francesa|pg Papúa Nueva Guinea|ph Filipinas|pk Pakistán|pl Polonia
pm San Pedro y Miquelón|pn Islas Pitcairn|pr Puerto Rico|ps Territorios Palestinos
pt Portugal|pw Palaos|py Paraguay|qa Catar|re Reunión|ro Rumanía|rs Serbia|ru Rusia
rw Ruanda|sa Arabia Saudí|sb Islas Salomón|sc Seychelles|sd Sudán|se Suecia
sg Singapur|sh Santa Elena|si Eslovenia|sj Svalbard y Jan Mayen|sk Eslovaquia
sl Sierra Leona|sm San Marino|sn Senegal|so Somalia|sr Surinam|ss Sudán del Sur
st Santo Tomé y Príncipe|sv El Salvador|sx Sint Maarten|sy Siria|sz Esuatini
tc Islas Turcas y Caicos|td Chad|tf Territorios Australes Franceses|tg Togo
th Tailandia|tj Tayikistán|tk Tokelau|tl Timor-Leste|tm Turkmenistán|tn Túnez
to Tonga|tr Turquía|tt Trinidad y Tobago|tv Tuvalu|tw Taiwán|tz Tanzania|ua Ucrania
ug Uganda|um Islas menores alejadas de EE. UU.|un Naciones Unidas|us Estados Unidos
uy Uruguay|uz Uzbekistán|va Ciudad del Vaticano|vc San Vicente y las Granadinas
ve Venezuela|vg Islas Vírgenes Británicas|vi Islas Vírgenes de EE. UU.|vn Vietnam
vu Vanuatu|wf Wallis y Futuna|ws Samoa|xk Kosovo|xx Desconocido|ye Yemen|yt Mayotte
za Sudáfrica|zm Zambia|zw Zimbabue"""

COUNTRY_NAMES: dict[str, str] = dict(
    entry.split(" ", 1) for line in _NAMES_ES.splitlines() for entry in line.split("|"))


_LOCALIZED = {"en": COUNTRY_NAMES_EN, "pt": COUNTRY_NAMES_PT, "fr": COUNTRY_NAMES_FR,
              "de": COUNTRY_NAMES_DE, "it": COUNTRY_NAMES_IT}


def country_name(code: str | None) -> str:
    """Nombre del código ISO alfa-2 en el idioma actual (o el código en mayúsculas)."""
    code = (code or "").strip()
    names = _LOCALIZED.get(i18n.current_language(), COUNTRY_NAMES)
    return names.get(code.lower(), code.upper())

