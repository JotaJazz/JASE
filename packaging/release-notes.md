# JASE 1.1.0

**Jimmy's All Seeing Eye** is a game server browser in the spirit of the classic All-Seeing Eye. It queries the master servers, lists servers with name, map, players and ping, and launches the game already connected with a double click.

## What's new in 1.1.0

**Game list**
- **GoldSrc** is now the default theme for new installs (your saved theme is kept).
- **Installed games only:** a checkbox above the game list hides the games you have not set up (Steam games always show, since they need no folder).
- **Countries folder** in the game list, like the classic ASE: under each game, next to *Mods* and *Maps*, tick one or more countries (with flags and server counts) to filter the list.

**Games**
- **Wolfenstein: ET** finds **ET: Legacy** (`etl.exe`, installer folder and registry key), and prefers it when both clients share a folder.
- **Jedi Academy and Jedi Outcast from Steam:** joining a server no longer stops with *Application load error 5:0000065434* (the game is started the way Steam starts it). If OpenJK or JK2MV are in `GameData`, JASE uses them.

**Command line without a GUI (new)**
- **`jase-cli`** runs the same queries without the interface, for scripts, servers and game panels: `jase-cli games` and `jase-cli scan <game>`, with `--json` output under a versioned contract, errors per master, `--steam-key` (or `JASE_STEAM_KEY`), `--country`, `--timeout` and `--include-empty`. See `docs/cli.md`.
- `pip install jase` now installs only the query engine (standard library only); the desktop app is `pip install "jase[gui]"`. The installer, zip and AppImage are unchanged.

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
- Zandronum (Doom)

## Steam games

The Steam-listed games (L4D, L4D2, CS: Source, TF2, HL DM, DoD, TFC, Garry's Mod) need a free Steam Web API key (https://steamcommunity.com/dev/apikey), pasted in *File → Settings → General*. It is stored in the Windows Credential Manager.

## Downloads

- **`JASE-1.1.0-setup.exe` — installer (recommended).** Adds a Start menu shortcut (desktop shortcut optional) and an uninstaller in *Settings → Apps*. It installs just for you without admin rights, or for all users if you choose.
- **`JASE-1.1.0-win64.zip` — portable.** No install: unzip it anywhere and open `JASE.exe`.
- **`JASE-1.1.0-x86_64.AppImage` — Linux (experimental).** Make it executable (`chmod +x`) and run it. It browses servers, joins Steam games and opens the other games with Wine (configurable in *Settings → General*; experimental). Settings are stored in `~/.config/jimmys-ase`.

Either way, your settings are stored in `%APPDATA%\JimmysASE` (nothing is written next to the exe), and uninstalling keeps them.

Neither the installer nor the exe is code-signed, so Windows SmartScreen may warn you the first time. Click **More info → Run anyway**.

## Not yet verified on Windows

These points have not been verified on a real Windows install yet:

- Executables and launch arguments for JKA (Jedi Academy), SoF2, BF1942 (including the `+game` argument for mods), CS 1.6, and Jedi Outcast, Xonotic, Warsow, Warfork, Tremulous and Smokin' Guns (executable names and `+connect`), and D-Day (`+set game dday` with `quake2.exe`, `q2pro.exe`, `r1q2.exe` or `yquake2.exe`).
- Registry keys and Steam folders used by the automatic game search.
- Executable icons.
- Levelshot extraction from .pk3 files.
- New in 1.1: joining Jedi Academy and Jedi Outcast from Steam (`SteamAppId` environment variable) and finding ET: Legacy through its registry key.
- Since 1.0: launching Zandronum (`-iwad … -connect`), searching the local network (broadcast through the Windows firewall), the RCON console against real servers and the portable zip self-update (the `.cmd` script that replaces the folder).

## Reporting problems

Please open an issue at https://github.com/JotaJazz/JASE/issues and include the game, what you did and what happened.

## Credits and licenses

- IP to Country Lite by DB-IP, CC BY 4.0.
- Flags: flag-icons by Panayiotis Lipiridis, MIT.
- Built with PySide6 (Qt for Python, LGPL-3.0), keyring and Pillow.
- JASE is MIT licensed. The full texts (including third-party notices in `THIRD-PARTY.txt`) are in the `licenses` folder (inside the zip, or in the install folder).

<!-- ES -->

# JASE 1.1.0

**Jimmy's All Seeing Eye** es un navegador de servidores de juego al estilo del clásico All-Seeing Eye. Consulta los servidores maestros, lista los servidores con nombre, mapa, jugadores y ping, y lanza el juego ya conectado con doble clic.

## Novedades de la 1.1.0

**Lista de juegos**
- **GoldSrc** es el tema por defecto en instalaciones nuevas (si ya elegiste uno, se mantiene).
- **Solo juegos instalados:** una casilla sobre la lista de juegos oculta los que no tienes configurados (los de Steam siempre aparecen, porque no necesitan carpeta).
- **Carpeta «Países»** en la lista de juegos, como en el ASE clásico: bajo cada juego, junto a *Mods* y *Mapas*, marca uno o más países (con bandera y cantidad de servidores) para filtrar la lista.

**Juegos**
- **Wolfenstein: ET** detecta **ET: Legacy** (`etl.exe`, la carpeta y la clave de registro de su instalador) y lo prefiere cuando ambos clientes están en la misma carpeta.
- **Jedi Academy y Jedi Outcast de Steam:** conectarse a un servidor ya no termina en *Application load error 5:0000065434* (el juego se abre como lo abre Steam). Si OpenJK o JK2MV están en `GameData`, JASE los usa.

**Línea de comandos sin interfaz (nuevo)**
- **`jase-cli`** hace las mismas consultas sin la interfaz, para scripts, servidores y paneles de juego: `jase-cli games` y `jase-cli scan <juego>`, con salida `--json` bajo un contrato versionado, errores por master, `--steam-key` (o `JASE_STEAM_KEY`), `--country`, `--timeout` e `--include-empty`. Ver `docs/cli.md`.
- `pip install jase` ahora instala solo el motor de consultas (solo la biblioteca estándar); la app de escritorio es `pip install "jase[gui]"`. El instalador, el zip y la AppImage no cambian.

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
- Zandronum (Doom)

## Juegos de Steam

Los juegos listados por Steam (L4D, L4D2, CS: Source, TF2, HL DM, DoD, TFC, Garry's Mod) necesitan una clave gratuita de la API web de Steam (https://steamcommunity.com/dev/apikey), que se pega en *Archivo → Ajustes → General*. Se guarda en el Administrador de credenciales de Windows.

## Descargas

- **`JASE-1.1.0-setup.exe`: instalador (recomendado).** Crea un acceso directo en el menú Inicio (el del escritorio es opcional) y un desinstalador en *Configuración → Aplicaciones*. Se instala solo para ti sin permisos de administrador, o para todos los usuarios si lo eliges.
- **`JASE-1.1.0-win64.zip`: portátil.** Sin instalación: descomprímelo donde quieras y abre `JASE.exe`.
- **`JASE-1.1.0-x86_64.AppImage`: Linux (experimental).** Hazlo ejecutable (`chmod +x`) y ábrelo. Sirve para buscar servidores, unirse a juegos de Steam y abrir los demás juegos con Wine (configurable en *Ajustes → General*; experimental). La configuración se guarda en `~/.config/jimmys-ase`.

En ambos casos la configuración se guarda en `%APPDATA%\JimmysASE` (no se escribe nada junto al exe) y desinstalar no la borra.

Ni el instalador ni el exe están firmados, así que Windows SmartScreen puede mostrar un aviso la primera vez. Pulsa **Más información → Ejecutar de todas formas**.

## Sin verificar aún en Windows

Estos puntos aún no se han verificado en una instalación real de Windows:

- Ejecutables y argumentos de lanzamiento de JKA (Jedi Academy), SoF2, BF1942 (incluido el argumento `+game` de los mods), CS 1.6, y Jedi Outcast, Xonotic, Warsow, Warfork, Tremulous y Smokin' Guns (nombres de ejecutable y `+connect`), y D-Day (`+set game dday` con `quake2.exe`, `q2pro.exe`, `r1q2.exe` o `yquake2.exe`).
- Claves de registro y carpetas de Steam que usa la búsqueda automática de juegos.
- Iconos de los ejecutables.
- Extracción de levelshots desde los archivos .pk3.
- Nuevo en la 1.1: conectarse en Jedi Academy y Jedi Outcast de Steam (variable de entorno `SteamAppId`) y detectar ET: Legacy por su clave de registro.
- Desde la 1.0: lanzar Zandronum (`-iwad … -connect`), buscar en la red local (difusión a través del firewall de Windows), la consola RCON contra servidores reales y la actualización del zip portátil (el script `.cmd` que reemplaza la carpeta).

## Cómo reportar problemas

Abre un issue en https://github.com/JotaJazz/JASE/issues e indica el juego, qué hiciste y qué pasó.

## Créditos y licencias

- IP to Country Lite de DB-IP, CC BY 4.0.
- Banderas: flag-icons de Panayiotis Lipiridis, MIT.
- Hecho con PySide6 (Qt for Python, LGPL-3.0), keyring y Pillow.
- JASE es MIT. Los textos completos (con los avisos de terceros en `THIRD-PARTY.txt`) están en la carpeta `licenses` (dentro del zip o en la carpeta de instalación).
