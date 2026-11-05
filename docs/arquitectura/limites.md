# Límites (constitución de la arquitectura)

Reglas normativas. Romper una exige un ADR que la reemplace ([adr/](adr/README.md)).

## Dependencias permitidas

| Desde \ hacia | `protocols/` | `models` | `games` | núcleo | `maps/` | `ui/` | PySide6 | keyring / Pillow |
|---|---|---|---|---|---|---|---|---|
| `protocols/` | sí | sí | no | no | no | no | no | no |
| `scanner.py` | sí | sí | sí | `i18n` | no | no | no | no |
| núcleo (`config`, `masters`, `launcher`, `server_cache`, …) | sí | sí | sí | sí | no | solo `ui/favorites.py` | no | `passwords.py` usa keyring |
| `maps/` | no | sí | sí | `net`, `paths`, `fetch_meta` | sí | no | no | Pillow |
| `cli.py` y lo que importa | sí | sí | sí | sí | no | no | **no** | **no** |
| `ui/` | sí | sí | sí | sí | sí | sí | sí | sí |

Lo vigilan `tests/test_cli.py` (la CLI no carga PySide6, keyring ni Pillow) y mypy.

**Excepción aceptada:** `src/jase/ui/favorites.py` es lógica pura sin Qt (está en `ui/`
por historia). La importan `recent.py` y `favorites_io.py`. No puede ganar imports de Qt.

## Hilos

- Hilo de la UI: solo pinta y reacciona. Nada de red, DNS, lectura de carpetas de juegos,
  extracción de imágenes ni archivos grandes.
- Red: `QThread` con su propio bucle asyncio (`ui/scan_worker.py`).
- Disco, descargas y búsquedas de exes: hilos **daemon** (`ui/daemon.py`, cargadores en
  `ui/*_loader.py`), que nunca bloquean el cierre ([ADR 0003](adr/0003-hilos-daemon-y-relays.md)).
- Los resultados vuelven por señal a un relay **sin padre** creado en el hilo de la UI.
  Todo resultado tardío se descarta por generación (`_generation`, `_icon_generation`)
  o por identidad de la petición (`request is self._x_request`).
- La recolección de basura corre solo en el hilo de la UI ([ADR 0002](adr/0002-gc-en-el-hilo-de-ui.md)).
- En los mixins `ui/window_*.py`, nunca `@Slot` (no son QObject: segfault al destruir).

## Datos remotos (no confiables)

- Todo lo que manda un servidor o un master se valida en el parser: largos, rangos
  (`protocols.bounded_int`), forma. Un servidor raro se descarta, nunca tumba el escaneo.
- Texto remoto en la UI: siempre texto plano o escapado (`colored_text.plain_tip`,
  `Qt.TextFormat.PlainText`, `html.escape`).
- Lo que llega a la línea de comandos del juego se valida (`launcher.is_mod_name`,
  `launcher.is_wad_name`).
- La clave de Steam solo viaja a `api.steampowered.com` (`protocols/steamweb.py`).
- Enlaces `jase://`: análisis estricto (`jase_url.parse`) y confirmación del usuario.

## Configuración

- Cada campo se carga por separado con su valor por defecto; una config ilegible se
  copia a `.bad` antes de usar los valores por defecto.
- No se reescriben rutas guardadas por el usuario.

## Prohibido

- Índices de columna literales en la UI: siempre `COL_*` de `ui/server_model.py`.
- Textos visibles sin `tr()`/`tr_n()` o sin traducción en los seis idiomas.
- Colores nuevos fuera de los tokens del tema (`ui/theme.py`, en todos los temas).
- Un juego nuevo sin sondeo real de su master ([ADR 0005](adr/0005-juegos-con-sondeo-real.md)).
- Versionar archivos de los juegos.
