**Español** | [English](user-guide.md)

# Guía de uso de JASE

Todo lo que hace JASE, en detalle. Para una vista rápida, el [README](../../README.es.md); para los masters y notas de cada juego, [Juegos y masters](juegos-y-masters.md).

## Descarga

Descarga la última versión desde la [página de Releases](https://github.com/JotaJazz/JASE/releases). JASE consulta GitHub como mucho una vez al día (al abrir y luego cada hora) y muestra un enlace en la barra de estado cuando hay una versión más nueva (se desactiva en Ajustes → General); ver [Actualizar desde la app](#actualizar-desde-la-app). Hay dos descargas:

- **Instalador** (`JASE-<versión>-setup.exe`, recomendado): crea un acceso directo en el menú Inicio (el del escritorio es opcional) y un desinstalador en *Configuración → Aplicaciones*. Se instala solo para ti sin permisos de administrador, o para todos los usuarios si lo eliges.
- **Zip portátil** (`JASE-<versión>-win64.zip`): sin instalación; descomprímelo donde quieras y ejecuta `JASE.exe`.

En ambos casos la configuración se guarda en `%APPDATA%\JimmysASE` y desinstalar no la borra.

Ni el instalador ni el exe están firmados, así que Windows SmartScreen puede mostrar un aviso la primera vez. Pulsa **Más información → Ejecutar de todas formas**.

## Uso

En el primer arranque JASE ofrece buscar los juegos instalados. También puedes abrir **Archivo → Ajustes** e indicar a mano la carpeta de cada juego (la que contiene, según el juego: `CoD2MP_s.exe` en CoD2, `CoDMP.exe` en CoD, `CoDUOMP.exe` en United Offensive, `iw3mp.exe` en CoD4, `quake3.exe` en Quake III, `WolfMP.exe` en RtCW, `ET.exe` en Enemy Territory, `System\UnrealTournament.exe` en UT99, `System\TacticalOps.exe` en Tactical Ops (o la carpeta que contiene `TO340\` y `TO350\`), `MOHAA.exe` en Medal of Honor, `moh_spearhead.exe` en Spearhead y `moh_breakthrough.exe` en Breakthrough (ver [Juegos y masters](juegos-y-masters.md#medal-of-honor-y-openmohaa)), `GameData\jk2mp.exe` en Jedi Outcast, `GameData\jamp.exe` en Jedi Academy, `SoF2MP.exe` en SoF2, `Quake3-UrT.exe` en Urban Terror, `openarena.exe` en OpenArena, `xonotic.exe` en Xonotic, `warsow_x64.exe` en Warsow, `warfork_x64.exe` en Warfork, `tremulous.exe` en Tremulous, `smokinguns.exe` en Smokin' Guns, `hl.exe` en CS 1.6, HL DM, DoD y TFC (los juegos Source como L4D, L4D2, CS: Source, TF2 y Garry's Mod se conectan con `steam://connect` y no necesitan carpeta; sus ejecutables son `left4dead.exe`, `left4dead2.exe`, `hl2.exe`, `tf_win64.exe` y `hl2.exe` o `bin\win64\gmod.exe`), `BF1942.exe` en BF1942, `quake2.exe` en Quake II, la carpeta de Quake II que tiene `dday\` en D-Day (`quake2.exe`, `q2pro.exe`, `r1q2.exe` o `yquake2.exe`), `System\UT2004.exe` en UT2004 o `zandronum.exe` en Zandronum).

Elige el juego en el panel izquierdo, pulsa **Refrescar lista** (icono de la barra o `F5`) y haz doble clic en un servidor para entrar.

- **Filtros rápidos** (panel izquierdo): *Ocultar vacíos*, *Ocultar llenos*, *Ocultar con contraseña*, *Ocultar solo bots*, *Ping máx.* y *País* (los países de la lista cargada, con cuántos servidores tiene cada uno); se combinan con el filtro de texto y el de versión, y se recuerdan entre sesiones. La columna *Jugadores* separa personas y bots (`2+6/16`: 2 personas y 6 bots).
- **Atajos:** `Enter` conecta al servidor seleccionado, `Ctrl+C` copia su IP:puerto (ambos con la tabla enfocada), `Ctrl+F` enfoca el filtro de texto, `Ctrl+Shift+F` busca un jugador en las listas cargadas y `F5` refresca.
- **Menú contextual** (clic derecho en un servidor): Conectar, Copiar IP:puerto, Refrescar este servidor, Agregar/Quitar de favoritos y Avisar cuando haya sitio; en Recientes, también Quitar de recientes.
- **Ficha del servidor:** al seleccionar uno se ve su imagen de mapa, país, jugadores y reglas.
- **Favoritos:** la estrella de la ficha (o el menú contextual) marca un servidor como favorito; **★ Favoritos** en el panel izquierdo los lista por juego, consultándolos directamente (sin master), y muestra los que no responden como sin conexión.
- **Agregar servidor:** **Servidor → Agregar servidor…** (`Ctrl+N`, también al final del menú contextual) abre un diálogo con juego, IP y puerto; al pegar `IP:puerto` (o un enlace `jase://`) el puerto se completa solo, y si el portapapeles ya tiene una dirección viene rellenado. **Ctrl+V** en la tabla de servidores lo abre con lo copiado. Al pegar varias direcciones (una por línea, o mezcladas con texto de Discord o de un foro) pasa a modo lista y las agrega todas de una vez. **Detectar** consulta la dirección con cada protocolo y elige el juego que responde (si encajan varios, los nombra). JASE consulta los servidores y deja seleccionado el primero. Abierto desde **★ Favoritos** (o con clic derecho sobre ella en el panel izquierdo) quedan guardados como favoritos, aunque no respondan. En los juegos GameSpy (UT, MOHAA, BF1942) también vale el puerto de juego: JASE prueba además el puerto de consulta que le corresponde.
- **Recientes:** **🕘 Recientes** en el panel izquierdo lista los servidores a los que te conectaste (hasta 20 por juego, del más reciente al más antiguo), consultados directamente como los favoritos; clic derecho → *Quitar de recientes* quita uno.
- **Grupos de favoritos:** clic derecho en un favorito → **Grupo** → *Nuevo grupo…* (o uno existente) los ordena en grupos como «Clan», «LAN» o «Probar». En **★ Favoritos** aparece un selector de grupo en la barra: muestra (y consulta) solo ese grupo, y *Copiar favoritos como texto* copia solo lo que se ve. Los grupos viajan en la exportación de favoritos.
- **Notas de favoritos:** clic derecho en un favorito (o **Servidor → Editar nota…**) para escribir una nota; se ve en la ficha del servidor y en el tooltip del nombre.
- **Exportar / importar favoritos:** **Archivo → Exportar favoritos…** / **Importar favoritos…** los guardan y cargan en un archivo JSON (con las notas). Al importar se combinan con tus favoritos actuales y se descartan las entradas no válidas. **Archivo → Copiar favoritos como texto** (o clic derecho en **★ Favoritos**) copia los del juego actual como líneas `IP:puerto  nombre`, para pegarlos en Discord; pegados en JASE se vuelven a agregar.
- **Filtros guardados:** el botón **Filtros guardados**, bajo los filtros rápidos, guarda la combinación actual con un nombre, aplica una guardada o la borra.
- **Avisar cuando haya sitio:** en un servidor lleno, clic derecho (o menú **Servidor**) → *Avisar cuando haya sitio* lo consulta cada 10 s durante hasta 30 min y, al liberarse un hueco, pregunta si quieres conectar. Solo se vigila un servidor a la vez; *Dejar de vigilar* lo cancela.
- **Avisar cuando se arme partida:** clic derecho en un servidor (o menú **Servidor**) → *Avisar cuando se arme partida…* pide cuántos jugadores esperas (4 por defecto; 0 lo desactiva). JASE lo revisa en segundo plano cada 2 minutos y avisa una vez cuando llega a esa cifra; solo cuentan las personas, no los bots. Vuelve a avisar si el servidor se vacía a la mitad y se llena de nuevo.
- **Árbol de juegos y barra de herramientas:** el panel izquierdo es un árbol por juego con sus **Mods** y **Mapas** (marcarlos filtra la tabla; *Quitar filtros de mod/mapa* los limpia). La barra de herramientas y el menú dan acceso a refrescar, detener, conectar, favoritos, ajustes y tema; *Buscar juegos…* está en el menú **Archivo**.
- **Red local (LAN):** **Servidor → Buscar en la red local** (`Ctrl+L`) busca servidores del juego actual en tu red (casa, LAN party, cibercafé) sin pasar por el master: manda la consulta por difusión a los puertos típicos del juego y lista los que contestan. Esa lista no reemplaza la de internet; `F5` vuelve a ella.
- **Ahora se juega:** **Servidor → Ahora se juega…** (`Ctrl+J`) junta en una lista los servidores con gente de todos tus juegos listos (con carpeta configurada o que conectan por Steam), de más a menos jugadores sin contar bots. Usa las listas ya cargadas en la sesión y escanea las demás en segundo plano (tres juegos a la vez); *Actualizar* vuelve a escanearlas todas. Doble clic en una fila muestra ese servidor en su juego.
- **Consola RCON** (para quien administra el servidor): **Servidor → Consola RCON…** (`Ctrl+Shift+R`) manda comandos de administrador al servidor seleccionado (cambiar de mapa, expulsar, `status`…) y muestra la respuesta; flechas arriba/abajo recorren el historial. Funciona con los juegos Quake 3 y Quake II, con Half-Life 1 (CS 1.6, DoD, TFC, HL DM) y con los de Source (CS: Source, TF2, L4D, Garry's Mod; RCON por TCP en el puerto del juego). La clave puede guardarse en el Administrador de credenciales de Windows.
- **Amigos:** **Servidor → Amigos…** (`Ctrl+Shift+A`) gestiona una lista de nombres de jugador; también puedes usar *Agregar a amigos* en el menú contextual de la tabla de jugadores. Los amigos salen en negrita en la tabla de jugadores y JASE avisa cuando uno aparece en un servidor que consultas. Opcionalmente (en el mismo diálogo) revisa tus favoritos y recientes en segundo plano cada 2 minutos.
- **Bandeja del sistema:** con **Ajustes → General → Minimizar a la bandeja del sistema** la ventana se va a la bandeja en vez de la barra de tareas; los avisos (sitio libre, amigos) salen entonces desde la bandeja.
- **Enlaces jase://:** *Copiar enlace jase://* en el menú contextual copia un enlace al servidor. Al abrir uno, se inicia JASE (o se usa la instancia abierta), consulta el servidor y pregunta antes de conectar. El instalador registra el protocolo; el zip portátil no.
- **Historial de ocupación:** los favoritos y recientes muestran en la ficha un gráfico del promedio de jugadores por hora (últimos 14 días, guardado en `%APPDATA%\JimmysASE\cache\history.json`).
- **Ventana:** el tamaño, las divisiones, las columnas y el último juego se recuerdan.

### Buscar juegos

**Buscar juegos…** (menú **Archivo** o **Archivo → Ajustes**) detecta los juegos instalados en segundo plano: registro de Windows, bibliotecas de Steam y carpetas típicas (`Program Files`, `Juegos`, `Games`…); con **Buscar en carpeta…** se recorre además una carpeta a elección. Cada juego se muestra como *nuevo*, *ya configurado* o *ruta guardada no encontrada*. Nunca se reemplaza una ruta que ya funciona salvo que se marque su casilla, y solo se aplican las filas marcadas. En el primer arranque, sin juegos configurados, se ofrece buscar una sola vez.

### Masters

La lista oficial de masters se actualiza sola desde GitHub (como mucho una vez al día; una descarga inválida se ignora y se conserva la última buena). Tus masters personales se guardan en `masters.custom.json`, junto a `config.json`. Se editan en **Archivo → Ajustes → Masters**: agregar masters (`host:puerto`), quitar los propios, desmarcar los oficiales, probarlos y buscar actualizaciones de la lista a mano.

### Refresco automático

En **Archivo → Ajustes → Refrescar automáticamente** puedes elegir que la lista se vuelva a cargar sola cada 2, 5, 10 o 15 minutos (por defecto, desactivado). No actúa mientras hay un escaneo en curso, un diálogo abierto o la ventana minimizada, mantiene seleccionado el servidor elegido y, en Favoritos o Recientes, vuelve a consultar esos servidores sin pasar por el master.

### Actualizar desde la app

En Windows, al pulsar *Nueva versión X disponible* en la barra de estado se ofrece descargar y ejecutar el instalador (se verifica su SHA-256 contra el publicado en GitHub) o abrir la página de la versión. En el zip portátil, *Descargar e instalar* baja el zip nuevo (también verificado), lo descomprime, cierra JASE, reemplaza la carpeta y vuelve a abrirlo; la configuración no se toca porque vive en `%APPDATA%`. **Ayuda → Buscar actualizaciones…** consulta GitHub en el momento (aunque el aviso automático esté apagado) y dice si ya tienes la última versión.

### Idioma

La interfaz está en español, inglés, portugués (Brasil), francés, alemán e italiano. Por defecto sigue el idioma de Windows; puedes elegir otro en **Archivo → Ajustes → Idioma**. El cambio se aplica al reiniciar JASE.

### Juegos de Steam y la clave de la API web de Steam

Left 4 Dead, Left 4 Dead 2, Counter-Strike: Source, Team Fortress 2, Half-Life Deathmatch, Day of Defeat, Team Fortress Classic y Garry's Mod se listan por la API web de Steam, que necesita una clave gratuita:

1. Consíguela en https://steamcommunity.com/dev/apikey. Sirve cualquier dominio (p. ej. `localhost`). Según las reglas de Steam, la cuenta necesita Steam Guard y un gasto mínimo.
2. Pégala en **Archivo → Ajustes → General → Clave de API de Steam**.

La clave se guarda en el Administrador de credenciales de Windows (servicio `JimmysAllSeeingEye`), nunca en `config.json`. Sin ella, los juegos de Steam muestran un aviso en lugar de la lista de servidores. Por defecto solo se listan servidores no vacíos; **Ajustes → General → Incluir servidores vacíos de Steam** lista también los vacíos (una lista mucho más larga). L4D, L4D2, CS: Source, TF2 y Garry's Mod se conectan por Steam (`steam://connect`), así que no necesitan carpeta de instalación; HL DM, DoD y TFC se lanzan con `hl.exe`, como CS 1.6.

## Más detalles

### Imágenes de mapas

- **CoD2:** se leen de los `.iwd` del juego instalado (verificado). En los demás CoD se intenta lo mismo, sin garantía. Quake 3, Open Arena, RtCW, Enemy Territory, Urban Terror, Jedi Outcast, Jedi Academy, Soldier of Fortune II, Warsow, Warfork, Tremulous y Smokin' Guns leen `levelshots/` de los `.pk3` de la carpeta del juego, sin garantía. Jedi Outcast, Jedi Academy, Xonotic, Warsow, Warfork, Tremulous y Smokin' Guns no tienen imágenes en GameTracker (sondeo del 2026-10-24); Xonotic no muestra imagen de mapa.
- **UT99 y Tactical Ops:** se generan desde los `.unr` de la carpeta `Maps` del juego.
- **GameTracker (opcional, desactivado por defecto):** lo usan CoD, CoD United Offensive, CoD4, Quake 3, Open Arena, RtCW, Enemy Territory, Urban Terror, SoF2, UT99, UT2004, MOHAA y sus expansiones, Counter-Strike 1.6, Counter-Strike: Source, Team Fortress 2 y Classic, Left 4 Dead 1 y 2, Half-Life Deathmatch, Day of Defeat, Garry's Mod, Battlefield 1942, Quake II y D-Day (sus mapas `dday1`…`dday5` están en la carpeta de Quake II de GameTracker). RtCW tiene cobertura parcial: su carpeta es la de Enemy Territory y solo trae los mapas que comparten ambos juegos. Jedi Academy usa solo archivos locales. En **Archivo → Ajustes** se puede activar la descarga de imágenes desde `image.gametracker.com` para los mapas que no están instalados. Se guardan en `%APPDATA%\JimmysASE\cache\maps`. Sin esa opción, JASE no descarga ninguna imagen.

### País de los servidores

La columna **País** muestra la bandera del país de cada servidor (nombre en el tooltip y en la ficha) y el filtro de texto acepta el código de país (`cl`, `br`…). El país se busca sin conexión, por la IPv4 del servidor, en la base «IP to Country Lite» de DB-IP. La primera vez que se abre JASE (y mientras falte el archivo) se descarga una sola vez en segundo plano a `%APPDATA%\JimmysASE\cache\dbip-country-lite.csv` (~4,5 MB comprimidos, ~30 MB en disco). Si no hay red, la columna queda vacía y solo se avisa en la barra de estado. **Ajustes → Actualizar base de países** la descarga de nuevo (DB-IP la publica cada mes).

### Configuración y contraseñas

- La configuración (rutas de los juegos, tema, filtro de versión, filtros rápidos, filtros guardados, favoritos y sus notas, amigos, opción de bandeja, ventana y masters) se guarda en `%APPDATA%\JimmysASE\config.json`. Se puede editar a mano con la app cerrada. Los masters propios (agregados o desactivados en *Ajustes → Masters*) van en `masters.custom.json`, junto a ella.
- Las contraseñas de servidores **no** se guardan en ese archivo: se almacenan en el **Administrador de credenciales de Windows** (servicio `JimmysAllSeeingEye`). Se pueden ver o borrar en *Panel de control → Administrador de credenciales → Credenciales de Windows*.
- Caché en `%APPDATA%\JimmysASE\cache\`: imágenes de mapas (`maps\`), la base de países (`dbip-country-lite.csv`), el historial de ocupación (`history.json`), la última lista de servidores de cada juego (`servers\`), la lista de masters descargada (`masters.json`) y las descargas de actualizaciones (`updates\`).

## Instalar y ejecutar desde el código

Requisitos: Windows 10/11 y **Python 3.11 o superior para Windows** (de [python.org](https://www.python.org/downloads/windows/); marca «Add python.exe to PATH» y deja instalado el lanzador `py`). Dependencias de Python de la app (el extra `gui`, se instala con `pip install -e ".[gui]"`): PySide6, keyring y Pillow (lee las capturas de mapas de los archivos del juego).

Desde la carpeta del proyecto, en PowerShell:

```powershell
py -m venv .venv
.venv\Scripts\pip install -e ".[gui]"
.venv\Scripts\python -m jase
```

(También queda disponible el comando `.venv\Scripts\jase`.) En Linux abre los juegos de Windows con Wine (el comando se elige en *Ajustes → General → Abrir juegos con*, p. ej. `wine` o `umu-run`; experimental); en WSL corre para desarrollo.

### Línea de comandos sin interfaz

`jase-cli` hace las mismas consultas sin la interfaz, para scripts, servidores o paneles. `pip install .` (sin extras) instala solo el motor de consultas, que usa únicamente la biblioteca estándar:

```sh
pip install .
jase-cli games
jase-cli scan cod2 --country
jase-cli scan l4d2 --json --steam-key TU_CLAVE   # o la variable JASE_STEAM_KEY
```

Con `--json` la salida sigue un contrato versionado (`"schema": 1`) con errores estructurados por master; ver [docs/cli.md](../cli.md) (en inglés).

### Tests

```powershell
.venv\Scripts\pip install -e ".[dev]"
.venv\Scripts\python -m pytest
```

Para comprobar que los masters reales responden (consulta todos los servidores listados; para una prueba ligera usa `--sample N` y, si quieres, ids de juego):

```powershell
.venv\Scripts\python scripts\probe_masters.py --sample 10 cod2 ut99
.venv\Scripts\python scripts\probe_masters.py
```

Arquitectura, convenciones y notas para colaborar: [docs/README.md](../README.md).

## Créditos

- IP to Country Lite de [DB-IP](https://db-ip.com), CC BY 4.0 (https://creativecommons.org/licenses/by/4.0/).
- Banderas: [flag-icons](https://github.com/lipis/flag-icons) 7.5.0 de Panayiotis Lipiridis, licencia MIT (`src/jase/assets/flags/LICENSE`).
- Hecho con PySide6 (Qt for Python, LGPL-3.0), keyring y Pillow.
- Masters y proyectos comunitarios que mantienen vivos estos juegos: 333networks, OpenSpy, jkhub, 1fxmod, dpmaster y cs16.net.
