# JASE 1.3.0

**Jimmy's All Seeing Eye** is a game server browser in the spirit of the classic All-Seeing Eye. It queries the master servers, lists servers with name, map, players and ping, and launches the game already connected with a double click.

## What's new in 1.3.0

**New**
- When you open JASE, each game shows its last full server list right away, like the original All-Seeing Eye, so you can join a server you know without waiting. The status bar says when the list was saved; press F5 to refresh it.
- Automatic refresh now updates the list in place instead of emptying it. Servers that stopped answering disappear at the end, and the view no longer jumps back to the selected row.
- New master servers, checked live: `dpmaster.deathmask.net` for Quake III Arena and `master.ouned.de` for Jedi Outcast.
- Opening JASE a second time brings the running window to the front instead of opening another one.

**Fixes**
- Two open JASE windows could overwrite each other's favorites when closing; now there is only one.
- Favorites added with *Add server* are saved even if you switch games while the server is being queried.
- Saved filters no longer hide favorites in the Favorites and Recent views.
- Refreshing one server after stopping a scan no longer shortens the saved list.
- *Settings* and *Only installed games* look for the game executables in the background, so a slow or disconnected drive no longer freezes the window.
- The RCON console recognizes the Quake II "bad rcon_password" reply, and no longer mistakes normal output that mentions a password for an error.
- The same server saved under a host name and an IP no longer gets mixed-up replies.

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
- Tactical Ops
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

- **`JASE-1.3.0-setup.exe` — installer (recommended).** Adds a Start menu shortcut (desktop shortcut optional) and an uninstaller in *Settings → Apps*. It installs just for you without admin rights, or for all users if you choose.
- **`JASE-1.3.0-win64.zip` — portable.** No install: unzip it anywhere and open `JASE.exe`.
- **`JASE-1.3.0-x86_64.AppImage` — Linux (experimental).** Make it executable (`chmod +x`) and run it. It browses servers, joins Steam games and opens the other games with Wine (configurable in *Settings → General*; experimental). Settings are stored in `~/.config/jimmys-ase`.

Either way, your settings are stored in `%APPDATA%\JimmysASE` (nothing is written next to the exe), and uninstalling keeps them.

Neither the installer nor the exe is code-signed, so Windows SmartScreen may warn you the first time. Click **More info → Run anyway**.

## Not yet verified on Windows

These points have not been verified on a real Windows install yet:

- Executables and launch arguments for JKA (Jedi Academy), SoF2, BF1942 (including the `+game` argument for mods), CS 1.6, and Jedi Outcast, Xonotic, Warsow, Warfork, Tremulous and Smokin' Guns (executable names and `+connect`), and D-Day (`+set game dday` with `quake2.exe`, `q2pro.exe`, `r1q2.exe` or `yquake2.exe`).
- Registry keys and Steam folders used by the automatic game search.
- Executable icons.
- Levelshot extraction from .pk3 files.
- New in 1.3: opening JASE a second time with the first one minimized to the tray or behind other windows (Windows may only flash its taskbar button).
- New in 1.2: joining Tactical Ops (`host:port?password=`) and finding it through the `Infogrames` and `TacticalOps` folders.
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

# JASE 1.3.0

**Jimmy's All Seeing Eye** es un navegador de servidores de juego al estilo del clásico All-Seeing Eye. Consulta los servidores maestros, lista los servidores con nombre, mapa, jugadores y ping, y lanza el juego ya conectado con doble clic.

## Novedades de la 1.3.0

**Nuevo**
- Al abrir JASE, cada juego muestra al instante su última lista completa de servidores, como el All-Seeing Eye original, para entrar directo a un servidor conocido sin esperar. La barra de estado dice de cuándo es la lista; F5 la refresca.
- El refresco automático ahora actualiza la lista en su sitio en vez de vaciarla. Los servidores que dejaron de responder desaparecen al final, y la vista ya no salta a la fila elegida.
- Masters nuevos, comprobados en vivo: `dpmaster.deathmask.net` para Quake III Arena y `master.ouned.de` para Jedi Outcast.
- Abrir JASE por segunda vez trae al frente la ventana abierta en vez de abrir otra.

**Correcciones**
- Dos ventanas de JASE abiertas podían pisarse los favoritos al cerrarse; ahora hay una sola.
- Los favoritos agregados con *Agregar servidor* se guardan aunque cambies de juego mientras se consulta el servidor.
- Los filtros guardados ya no ocultan favoritos en las vistas Favoritos y Recientes.
- Refrescar un servidor tras detener una búsqueda ya no recorta la lista guardada.
- *Ajustes* y *Solo juegos instalados* buscan los ejecutables en segundo plano, así que una unidad lenta o desconectada ya no congela la ventana.
- La consola RCON reconoce la respuesta «bad rcon_password» de Quake II y ya no confunde con un error una salida normal que menciona una contraseña.
- Un mismo servidor guardado con nombre de host y con IP ya no recibe respuestas cruzadas.

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
- Tactical Ops
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

- **`JASE-1.3.0-setup.exe`: instalador (recomendado).** Crea un acceso directo en el menú Inicio (el del escritorio es opcional) y un desinstalador en *Configuración → Aplicaciones*. Se instala solo para ti sin permisos de administrador, o para todos los usuarios si lo eliges.
- **`JASE-1.3.0-win64.zip`: portátil.** Sin instalación: descomprímelo donde quieras y abre `JASE.exe`.
- **`JASE-1.3.0-x86_64.AppImage`: Linux (experimental).** Hazlo ejecutable (`chmod +x`) y ábrelo. Sirve para buscar servidores, unirse a juegos de Steam y abrir los demás juegos con Wine (configurable en *Ajustes → General*; experimental). La configuración se guarda en `~/.config/jimmys-ase`.

En ambos casos la configuración se guarda en `%APPDATA%\JimmysASE` (no se escribe nada junto al exe) y desinstalar no la borra.

Ni el instalador ni el exe están firmados, así que Windows SmartScreen puede mostrar un aviso la primera vez. Pulsa **Más información → Ejecutar de todas formas**.

## Sin verificar aún en Windows

Estos puntos aún no se han verificado en una instalación real de Windows:

- Ejecutables y argumentos de lanzamiento de JKA (Jedi Academy), SoF2, BF1942 (incluido el argumento `+game` de los mods), CS 1.6, y Jedi Outcast, Xonotic, Warsow, Warfork, Tremulous y Smokin' Guns (nombres de ejecutable y `+connect`), y D-Day (`+set game dday` con `quake2.exe`, `q2pro.exe`, `r1q2.exe` o `yquake2.exe`).
- Claves de registro y carpetas de Steam que usa la búsqueda automática de juegos.
- Iconos de los ejecutables.
- Extracción de levelshots desde los archivos .pk3.
- Nuevo en la 1.3: abrir JASE por segunda vez con la primera minimizada en la bandeja o tapada por otras ventanas (Windows puede solo hacer parpadear su botón en la barra de tareas).
- Nuevo en la 1.2: conectarse en Tactical Ops (`host:port?password=`) y detectarlo por las carpetas `Infogrames` y `TacticalOps`.
- Nuevo en la 1.1: conectarse en Jedi Academy y Jedi Outcast de Steam (variable de entorno `SteamAppId`) y detectar ET: Legacy por su clave de registro.
- Desde la 1.0: lanzar Zandronum (`-iwad … -connect`), buscar en la red local (difusión a través del firewall de Windows), la consola RCON contra servidores reales y la actualización del zip portátil (el script `.cmd` que reemplaza la carpeta).

## Cómo reportar problemas

Abre un issue en https://github.com/JotaJazz/JASE/issues e indica el juego, qué hiciste y qué pasó.

## Créditos y licencias

- IP to Country Lite de DB-IP, CC BY 4.0.
- Banderas: flag-icons de Panayiotis Lipiridis, MIT.
- Hecho con PySide6 (Qt for Python, LGPL-3.0), keyring y Pillow.
- JASE es MIT. Los textos completos (con los avisos de terceros en `THIRD-PARTY.txt`) están en la carpeta `licenses` (dentro del zip o en la carpeta de instalación).
