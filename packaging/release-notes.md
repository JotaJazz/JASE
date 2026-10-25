# JASE 0.12.0 (beta)

**Jimmy's All Seeing Eye** is a game server browser in the spirit of the classic All-Seeing Eye. It queries the master servers, lists servers with name, map, players and ping, and launches the game already connected with a double click.

## What's new in 0.12.0

- Notes on favorite servers (right-click or Server → Edit note…), shown in the card and the name tooltip.
- Export and import favorites as JSON (File menu), notes included.
- Saved filters: name and reuse filter combinations from the quick filters panel.
- Minimize to the system tray (Settings → General); notifications are shown from the tray.
- Friends (Server → Friends…, Ctrl+Shift+A, or Add to friends in the players table): shown in bold, with a notification when they appear and an optional background check of favorites and recent servers every 2 minutes.
- jase:// links (Copy jase:// link): opening one starts JASE, queries the server and asks before connecting. The installer registers the protocol; the portable zip does not.
- Occupancy history: favorites and recent servers show an hourly average chart (last 14 days).
- In-app update: on Windows, the «New version available» link offers to download and run the installer (SHA-256 verified) or open the page. The update check now runs every hour (still at most one request a day).
- 6 new games: Jedi Outcast, Xonotic, Warsow, Warfork, Tremulous and Smokin' Guns.
- Quality: mypy in CI and Dependabot for GitHub Actions.

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

- **`JASE-0.12.0-setup.exe` — installer (recommended).** Adds a Start menu shortcut (desktop shortcut optional) and an uninstaller in *Settings → Apps*. It installs just for you without admin rights, or for all users if you choose.
- **`JASE-0.12.0-win64.zip` — portable.** No install: unzip it anywhere and open `JASE.exe`.
- **`JASE-0.12.0-x86_64.AppImage` — Linux (experimental).** Make it executable (`chmod +x`) and run it. It browses servers and joins Steam games; launching the other games is not supported on Linux yet. Settings are stored in `~/.config/jimmys-ase`.

Either way, your settings are stored in `%APPDATA%\JimmysASE` (nothing is written next to the exe), and uninstalling keeps them.

Neither the installer nor the exe is code-signed, so Windows SmartScreen may warn you the first time. Click **More info → Run anyway**.

## Beta: not yet verified on Windows

This is a pre-release. These points have not been verified on a real Windows install:

- Executables and launch arguments for JKA (Jedi Academy), SoF2, BF1942 (including the `+game` argument for mods), CS 1.6, and Jedi Outcast, Xonotic, Warsow, Warfork, Tremulous and Smokin' Guns (executable names and `+connect`).
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

# JASE 0.12.0 (beta)

**Jimmy's All Seeing Eye** es un navegador de servidores de juego al estilo del clásico All-Seeing Eye. Consulta los servidores maestros, lista los servidores con nombre, mapa, jugadores y ping, y lanza el juego ya conectado con doble clic.

## Novedades de la 0.12.0

- Notas en los servidores favoritos (clic derecho o Servidor → Editar nota…), visibles en la ficha y en el tooltip del nombre.
- Exportar e importar favoritos en JSON (menú Archivo), con las notas.
- Filtros guardados: nombra y reutiliza combinaciones de filtros desde el panel de filtros rápidos.
- Minimizar a la bandeja del sistema (Ajustes → General); los avisos salen desde la bandeja.
- Amigos (Servidor → Amigos…, Ctrl+Shift+A, o Agregar a amigos en la tabla de jugadores): en negrita, con aviso cuando aparecen y revisión opcional en segundo plano de favoritos y recientes cada 2 minutos.
- Enlaces jase:// (Copiar enlace jase://): al abrir uno se inicia JASE, consulta el servidor y pregunta antes de conectar. El instalador registra el protocolo; el zip portátil no.
- Historial de ocupación: los favoritos y recientes muestran un gráfico del promedio por hora (últimos 14 días).
- Actualización desde la app: en Windows, el enlace «Nueva versión disponible» ofrece descargar y ejecutar el instalador (SHA-256 verificado) o abrir la página. La comprobación se hace cada hora (como mucho una consulta al día).
- 6 juegos nuevos: Jedi Outcast, Xonotic, Warsow, Warfork, Tremulous y Smokin' Guns.
- Calidad: mypy en CI y Dependabot para las GitHub Actions.

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

- **`JASE-0.12.0-setup.exe`: instalador (recomendado).** Crea un acceso directo en el menú Inicio (el del escritorio es opcional) y un desinstalador en *Configuración → Aplicaciones*. Se instala solo para ti sin permisos de administrador, o para todos los usuarios si lo eliges.
- **`JASE-0.12.0-win64.zip`: portátil.** Sin instalación: descomprímelo donde quieras y abre `JASE.exe`.
- **`JASE-0.12.0-x86_64.AppImage`: Linux (experimental).** Hazlo ejecutable (`chmod +x`) y ábrelo. Sirve para buscar servidores y unirse a juegos de Steam; lanzar los demás juegos todavía no funciona en Linux. La configuración se guarda en `~/.config/jimmys-ase`.

En ambos casos la configuración se guarda en `%APPDATA%\JimmysASE` (no se escribe nada junto al exe) y desinstalar no la borra.

Ni el instalador ni el exe están firmados, así que Windows SmartScreen puede mostrar un aviso la primera vez. Pulsa **Más información → Ejecutar de todas formas**.

## Beta: sin verificar aún en Windows

Esta es una versión preliminar. Estos puntos no se han verificado en una instalación real de Windows:

- Ejecutables y argumentos de lanzamiento de JKA (Jedi Academy), SoF2, BF1942 (incluido el argumento `+game` de los mods), CS 1.6, y Jedi Outcast, Xonotic, Warsow, Warfork, Tremulous y Smokin' Guns (nombres de ejecutable y `+connect`).
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
