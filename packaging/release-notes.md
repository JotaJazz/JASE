# JASE 0.14.0

**Jimmy's All Seeing Eye** is a game server browser in the spirit of the classic All-Seeing Eye. It queries the master servers, lists servers with name, map, players and ping, and launches the game already connected with a double click.

## What's new in 0.14.0

- **Classic** now looks like Windows 98: dithered scrollbars with bevelled arrows, ✓ checkboxes, a flat toolbar that pops up on hover, 16-colour pixel-art icons and an 8 pt font.
- **Windows XP** now looks like Luna: blue-bordered buttons with an orange hover glow, light-blue scrollbars, green ✓ checkboxes, orange tab and header accents and Luna-style icons.
- Both draw the game tree with dotted lines and +/− boxes and show player occupancy as a block progress bar.
- The other themes got slim scrollbars, dimmed disabled buttons, hover feedback, visible checkboxes in Dark and Light, and arrows and tooltips in each theme's colours.
- Each themed look has its own touch: a sunset gradient in Synthwave, cyan accents in Neon, CRT scanlines in Amber, rust stripes in Rust, a warm gradient in Brown and a selection marker in Green.

## Supported games

- Call of Duty 2
- Call of Duty
- Call of Duty: United Offensive
- Call of Duty 4: Modern Warfare
- Quake III Arena
- Return to Castle Wolfenstein
- Wolfenstein: Enemy Territory
- Star Wars Jedi Knight II: Jedi Outcast
- Star Wars Jedi Knight: Jedi Academy
- Soldier of Fortune II
- Urban Terror
- OpenArena
- Xonotic
- Warsow
- Warfork
- Tremulous
- Smokin' Guns
- Unreal Tournament
- Medal of Honor: Allied Assault
- Medal of Honor: Spearhead
- Medal of Honor: Breakthrough
- Counter-Strike 1.6
- Left 4 Dead
- Left 4 Dead 2
- Counter-Strike: Source
- Team Fortress 2
- Half-Life Deathmatch
- Day of Defeat
- Team Fortress Classic
- Garry's Mod
- Battlefield 1942
- Quake II
- D-Day: Normandy
- Unreal Tournament 2004

## Steam games

The Steam-listed games (L4D, L4D2, CS: Source, TF2, HL DM, DoD, TFC, Garry's Mod) need a free Steam Web API key (https://steamcommunity.com/dev/apikey), pasted in *File → Settings → General*. It is stored in the Windows Credential Manager.

## Downloads

- **`JASE-0.14.0-setup.exe` — installer (recommended).** Adds a Start menu shortcut (desktop shortcut optional) and an uninstaller in *Settings → Apps*. It installs just for you without admin rights, or for all users if you choose.
- **`JASE-0.14.0-win64.zip` — portable.** No install: unzip it anywhere and open `JASE.exe`.
- **`JASE-0.14.0-x86_64.AppImage` — Linux (experimental).** Make it executable (`chmod +x`) and run it. It browses servers and joins Steam games; launching the other games is not supported on Linux yet. Settings are stored in `~/.config/jimmys-ase`.

Either way, your settings are stored in `%APPDATA%\JimmysASE` (nothing is written next to the exe), and uninstalling keeps them.

Neither the installer nor the exe is code-signed, so Windows SmartScreen may warn you the first time. Click **More info → Run anyway**.

## Not yet verified on Windows

These points have not been verified on a real Windows install yet:

- Executables and launch arguments for JKA (Jedi Academy), SoF2, BF1942 (including the `+game` argument for mods), CS 1.6, and Jedi Outcast, Xonotic, Warsow, Warfork, Tremulous and Smokin' Guns (executable names and `+connect`), and D-Day (`+set game dday` with `quake2.exe`, `q2pro.exe`, `r1q2.exe` or `yquake2.exe`).
- Registry keys and Steam folders used by the automatic game search.
- Executable icons.
- Levelshot extraction from .pk3 files.

## Reporting problems

Please open an issue at https://github.com/JotaJazz/JASE/issues and include the game, what you did and what happened.

## Credits and licenses

- IP to Country Lite by DB-IP, CC BY 4.0.
- Flags: flag-icons by Panayiotis Lipiridis, MIT.
- Built with PySide6 (Qt for Python, LGPL-3.0), keyring and Pillow.
- JASE is MIT licensed. The full texts (including third-party notices in `THIRD-PARTY.txt`) are in the `licenses` folder (inside the zip, or in the install folder).

<!-- ES -->

# JASE 0.14.0

**Jimmy's All Seeing Eye** es un navegador de servidores de juego al estilo del clásico All-Seeing Eye. Consulta los servidores maestros, lista los servidores con nombre, mapa, jugadores y ping, y lanza el juego ya conectado con doble clic.

## Novedades de la 0.14.0

- **Clásico** ahora se ve como Windows 98: scroll tramado con flechas biseladas, casillas con ✓, barra de herramientas plana que se levanta al pasar el mouse, iconos en pixel art de 16 colores y letra a 8 pt.
- **Windows XP** ahora se ve como Luna: botones con borde azul y brillo naranja al pasar el mouse, scroll celeste, casillas con ✓ verde, acentos naranjas en pestañas y encabezados, e iconos al estilo Luna.
- Ambos dibujan el árbol de juegos con líneas punteadas y cuadros +/−, y muestran la ocupación como una barra de progreso por bloques.
- Los demás temas estrenan scroll delgado, botones deshabilitados atenuados, respuesta al pasar el mouse, casillas visibles en Oscuro y Claro, y flechas y tooltips con los colores de cada tema.
- Cada tema con carácter tiene su detalle: degradado de atardecer en Synthwave, acentos cian en Neón, líneas de barrido CRT en Ámbar, franjas de óxido en Óxido, degradado cálido en Marrón y marca de selección en Verde.

## Juegos soportados

- Call of Duty 2
- Call of Duty
- Call of Duty: United Offensive
- Call of Duty 4: Modern Warfare
- Quake III Arena
- Return to Castle Wolfenstein
- Wolfenstein: Enemy Territory
- Star Wars Jedi Knight II: Jedi Outcast
- Star Wars Jedi Knight: Jedi Academy
- Soldier of Fortune II
- Urban Terror
- OpenArena
- Xonotic
- Warsow
- Warfork
- Tremulous
- Smokin' Guns
- Unreal Tournament
- Medal of Honor: Allied Assault
- Medal of Honor: Spearhead
- Medal of Honor: Breakthrough
- Counter-Strike 1.6
- Left 4 Dead
- Left 4 Dead 2
- Counter-Strike: Source
- Team Fortress 2
- Half-Life Deathmatch
- Day of Defeat
- Team Fortress Classic
- Garry's Mod
- Battlefield 1942
- Quake II
- D-Day: Normandy
- Unreal Tournament 2004

## Juegos de Steam

Los juegos listados por Steam (L4D, L4D2, CS: Source, TF2, HL DM, DoD, TFC, Garry's Mod) necesitan una clave gratuita de la API web de Steam (https://steamcommunity.com/dev/apikey), que se pega en *Archivo → Ajustes → General*. Se guarda en el Administrador de credenciales de Windows.

## Descargas

- **`JASE-0.14.0-setup.exe`: instalador (recomendado).** Crea un acceso directo en el menú Inicio (el del escritorio es opcional) y un desinstalador en *Configuración → Aplicaciones*. Se instala solo para ti sin permisos de administrador, o para todos los usuarios si lo eliges.
- **`JASE-0.14.0-win64.zip`: portátil.** Sin instalación: descomprímelo donde quieras y abre `JASE.exe`.
- **`JASE-0.14.0-x86_64.AppImage`: Linux (experimental).** Hazlo ejecutable (`chmod +x`) y ábrelo. Sirve para buscar servidores y unirse a juegos de Steam; lanzar los demás juegos todavía no funciona en Linux. La configuración se guarda en `~/.config/jimmys-ase`.

En ambos casos la configuración se guarda en `%APPDATA%\JimmysASE` (no se escribe nada junto al exe) y desinstalar no la borra.

Ni el instalador ni el exe están firmados, así que Windows SmartScreen puede mostrar un aviso la primera vez. Pulsa **Más información → Ejecutar de todas formas**.

## Sin verificar aún en Windows

Estos puntos aún no se han verificado en una instalación real de Windows:

- Ejecutables y argumentos de lanzamiento de JKA (Jedi Academy), SoF2, BF1942 (incluido el argumento `+game` de los mods), CS 1.6, y Jedi Outcast, Xonotic, Warsow, Warfork, Tremulous y Smokin' Guns (nombres de ejecutable y `+connect`), y D-Day (`+set game dday` con `quake2.exe`, `q2pro.exe`, `r1q2.exe` o `yquake2.exe`).
- Claves de registro y carpetas de Steam que usa la búsqueda automática de juegos.
- Iconos de los ejecutables.
- Extracción de levelshots desde los archivos .pk3.

## Cómo reportar problemas

Abre un issue en https://github.com/JotaJazz/JASE/issues e indica el juego, qué hiciste y qué pasó.

## Créditos y licencias

- IP to Country Lite de DB-IP, CC BY 4.0.
- Banderas: flag-icons de Panayiotis Lipiridis, MIT.
- Hecho con PySide6 (Qt for Python, LGPL-3.0), keyring y Pillow.
- JASE es MIT. Los textos completos (con los avisos de terceros en `THIRD-PARTY.txt`) están en la carpeta `licenses` (dentro del zip o en la carpeta de instalación).
