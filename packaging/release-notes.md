# JASE 0.9.0 (beta)

**Jimmy's All Seeing Eye** is a game server browser in the spirit of the classic All-Seeing Eye. It queries the master servers, lists servers with name, map, players and ping, and launches the game already connected with a double click.

## Supported games

- Call of Duty 2
- Call of Duty
- Call of Duty: United Offensive
- Call of Duty 4: Modern Warfare
- Quake III Arena
- Return to Castle Wolfenstein
- Wolfenstein: Enemy Territory
- Star Wars Jedi Knight: Jedi Academy
- Soldier of Fortune II
- Urban Terror
- OpenArena
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
- Battlefield 1942

## Downloads

- **`JASE-0.9.0-setup.exe` — installer (recommended).** Adds a Start menu shortcut (desktop shortcut optional) and an uninstaller in *Settings → Apps*. It installs just for you without admin rights, or for all users if you choose.
- **`JASE-0.9.0-win64.zip` — portable.** No install: unzip it anywhere and open `JASE.exe`.

Either way, your settings are stored in `%APPDATA%\JimmysASE` (nothing is written next to the exe), and uninstalling keeps them.

Neither the installer nor the exe is code-signed, so Windows SmartScreen may warn you the first time. Click **More info → Run anyway**.

## Beta: not yet verified on Windows

This is a pre-release. These points have not been verified on a real Windows install:

- Executables and launch arguments for JKA (Jedi Academy), SoF2, BF1942 (including the `+game` argument for mods) and CS 1.6.
- Registry keys and Steam folders used by the automatic game search.
- Executable icons.

## Reporting problems

Please open an issue at https://github.com/JotaJazz/JASE/issues and include the game, what you did and what happened.

## Credits and licenses

- IP to Country Lite by DB-IP, CC BY 4.0.
- Flags: flag-icons by Panayiotis Lipiridis, MIT.
- Built with PySide6 (Qt for Python, LGPL-3.0), keyring and Pillow.
- JASE is MIT licensed. The full texts (including third-party notices in `THIRD-PARTY.txt`) are in the `licenses` folder (inside the zip, or in the install folder).

<!-- ES -->

# JASE 0.9.0 (beta)

**Jimmy's All Seeing Eye** es un navegador de servidores de juego al estilo del clásico All-Seeing Eye. Consulta los servidores maestros, lista los servidores con nombre, mapa, jugadores y ping, y lanza el juego ya conectado con doble clic.

## Juegos soportados

- Call of Duty 2
- Call of Duty
- Call of Duty: United Offensive
- Call of Duty 4: Modern Warfare
- Quake III Arena
- Return to Castle Wolfenstein
- Wolfenstein: Enemy Territory
- Star Wars Jedi Knight: Jedi Academy
- Soldier of Fortune II
- Urban Terror
- OpenArena
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
- Battlefield 1942

## Descargas

- **`JASE-0.9.0-setup.exe`: instalador (recomendado).** Crea un acceso directo en el menú Inicio (el del escritorio es opcional) y un desinstalador en *Configuración → Aplicaciones*. Se instala solo para ti sin permisos de administrador, o para todos los usuarios si lo eliges.
- **`JASE-0.9.0-win64.zip`: portátil.** Sin instalación: descomprímelo donde quieras y abre `JASE.exe`.

En ambos casos la configuración se guarda en `%APPDATA%\JimmysASE` (no se escribe nada junto al exe) y desinstalar no la borra.

Ni el instalador ni el exe están firmados, así que Windows SmartScreen puede mostrar un aviso la primera vez. Pulsa **Más información → Ejecutar de todas formas**.

## Beta: sin verificar aún en Windows

Esta es una versión preliminar. Estos puntos no se han verificado en una instalación real de Windows:

- Ejecutables y argumentos de lanzamiento de JKA (Jedi Academy), SoF2, BF1942 (incluido el argumento `+game` de los mods) y CS 1.6.
- Claves de registro y carpetas de Steam que usa la búsqueda automática de juegos.
- Iconos de los ejecutables.

## Cómo reportar problemas

Abre un issue en https://github.com/JotaJazz/JASE/issues e indica el juego, qué hiciste y qué pasó.

## Créditos y licencias

- IP to Country Lite de DB-IP, CC BY 4.0.
- Banderas: flag-icons de Panayiotis Lipiridis, MIT.
- Hecho con PySide6 (Qt for Python, LGPL-3.0), keyring y Pillow.
- JASE es MIT. Los textos completos (con los avisos de terceros en `THIRD-PARTY.txt`) están en la carpeta `licenses` (dentro del zip o en la carpeta de instalación).
