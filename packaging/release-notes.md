# JASE 1.0.0

**Jimmy's All Seeing Eye** is a game server browser in the spirit of the classic All-Seeing Eye. It queries the master servers, lists servers with name, map, players and ping, and launches the game already connected with a double click.

## What's new in 1.0.0

JASE turns 1.0. Everything below is new since 1.0.0.

**Finding a game**
- **Playing now** (**Server → Playing now…**, `Ctrl+J`): the servers with people from all your ready games in one list, busiest first.
- **Local network** (**Server → Search the local network**, `Ctrl+L`): finds servers on your LAN without a master, like the classic ASE.
- **Notify me when a game gets going:** pick how many players you are waiting for on a server and JASE tells you when it gets there (checked in the background every 2 minutes).
- **People and bots apart:** the *Players* column shows `2+6/16` (2 people, 6 bots) and sorts by people; new quick filter *Hide bots only*.
- **Country filter** in the left panel, with the countries of the loaded list.

**Favorites**
- **Groups** (right-click → **Group**): sort favorites into "Clan", "LAN"… and show one group at a time in ★ Favorites.
- **Copy favorites as text** (**File** menu or right-click ★ Favorites) to paste them in Discord; pasted back into JASE they are added again.

**New game**
- **Zandronum (Doom multiplayer)**, a sixth protocol. The IWAD and PWADs must already be in Zandronum's folder.

**For server admins**
- **RCON console** (**Server → RCON console…**, `Ctrl+Shift+R`) for Quake 3, Quake II, Half-Life 1 and Source servers.

**More**
- The interface is now also in **Portuguese (Brazil), French, German and Italian**.
- **Help → Check for updates…** asks GitHub right away.
- The **portable zip** now updates itself from the app, like the installer.
- **Linux (experimental):** the AppImage opens Windows games with Wine (command in *Settings → General*).
- Fixed a rare crash on Windows when Python cleaned up memory from a network thread.

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

- **`JASE-1.0.0-setup.exe` — installer (recommended).** Adds a Start menu shortcut (desktop shortcut optional) and an uninstaller in *Settings → Apps*. It installs just for you without admin rights, or for all users if you choose.
- **`JASE-1.0.0-win64.zip` — portable.** No install: unzip it anywhere and open `JASE.exe`.
- **`JASE-1.0.0-x86_64.AppImage` — Linux (experimental).** Make it executable (`chmod +x`) and run it. It browses servers, joins Steam games and opens the other games with Wine (configurable in *Settings → General*; experimental). Settings are stored in `~/.config/jimmys-ase`.

Either way, your settings are stored in `%APPDATA%\JimmysASE` (nothing is written next to the exe), and uninstalling keeps them.

Neither the installer nor the exe is code-signed, so Windows SmartScreen may warn you the first time. Click **More info → Run anyway**.

## Not yet verified on Windows

These points have not been verified on a real Windows install yet:

- Executables and launch arguments for JKA (Jedi Academy), SoF2, BF1942 (including the `+game` argument for mods), CS 1.6, and Jedi Outcast, Xonotic, Warsow, Warfork, Tremulous and Smokin' Guns (executable names and `+connect`), and D-Day (`+set game dday` with `quake2.exe`, `q2pro.exe`, `r1q2.exe` or `yquake2.exe`).
- Registry keys and Steam folders used by the automatic game search.
- Executable icons.
- Levelshot extraction from .pk3 files.
- New in 1.0: launching Zandronum (`-iwad … -connect`), searching the local network (broadcast through the Windows firewall), the RCON console against real servers and the portable zip self-update (the `.cmd` script that replaces the folder).

## Reporting problems

Please open an issue at https://github.com/JotaJazz/JASE/issues and include the game, what you did and what happened.

## Credits and licenses

- IP to Country Lite by DB-IP, CC BY 4.0.
- Flags: flag-icons by Panayiotis Lipiridis, MIT.
- Built with PySide6 (Qt for Python, LGPL-3.0), keyring and Pillow.
- JASE is MIT licensed. The full texts (including third-party notices in `THIRD-PARTY.txt`) are in the `licenses` folder (inside the zip, or in the install folder).

<!-- ES -->

# JASE 1.0.0

**Jimmy's All Seeing Eye** es un navegador de servidores de juego al estilo del clásico All-Seeing Eye. Consulta los servidores maestros, lista los servidores con nombre, mapa, jugadores y ping, y lanza el juego ya conectado con doble clic.

## Novedades de la 1.0.0

JASE llega a la 1.0. Todo esto es nuevo desde la 1.0.0.

**Encontrar partida**
- **Ahora se juega** (**Servidor → Ahora se juega…**, `Ctrl+J`): los servidores con gente de todos tus juegos listos en una sola lista, de más a menos jugadores.
- **Red local** (**Servidor → Buscar en la red local**, `Ctrl+L`): busca servidores en tu LAN sin master, como el ASE clásico.
- **Avisar cuando se arme partida:** eliges cuántos jugadores esperas en un servidor y JASE te avisa cuando llega a esa cifra (lo revisa en segundo plano cada 2 minutos).
- **Personas y bots por separado:** la columna *Jugadores* muestra `2+6/16` (2 personas, 6 bots) y ordena por personas; filtro rápido nuevo *Ocultar solo bots*.
- **Filtro por país** en el panel izquierdo, con los países de la lista cargada.

**Favoritos**
- **Grupos** (clic derecho → **Grupo**): ordena los favoritos en «Clan», «LAN»… y muestra un grupo a la vez en ★ Favoritos.
- **Copiar favoritos como texto** (menú **Archivo** o clic derecho en ★ Favoritos) para pegarlos en Discord; pegados en JASE se vuelven a agregar.

**Juego nuevo**
- **Zandronum (Doom multijugador)**, un sexto protocolo. El IWAD y los PWAD tienen que estar ya en la carpeta de Zandronum.

**Para administradores de servidores**
- **Consola RCON** (**Servidor → Consola RCON…**, `Ctrl+Shift+R`) para servidores de Quake 3, Quake II, Half-Life 1 y Source.

**Además**
- La interfaz ahora también está en **portugués (Brasil), francés, alemán e italiano**.
- **Ayuda → Buscar actualizaciones…** consulta GitHub en el momento.
- El **zip portátil** ahora se actualiza solo desde la app, igual que el instalador.
- **Linux (experimental):** el AppImage abre los juegos de Windows con Wine (comando en *Ajustes → General*).
- Se corrigió un cierre inesperado poco frecuente en Windows cuando Python liberaba memoria desde un hilo de red.

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

- **`JASE-1.0.0-setup.exe`: instalador (recomendado).** Crea un acceso directo en el menú Inicio (el del escritorio es opcional) y un desinstalador en *Configuración → Aplicaciones*. Se instala solo para ti sin permisos de administrador, o para todos los usuarios si lo eliges.
- **`JASE-1.0.0-win64.zip`: portátil.** Sin instalación: descomprímelo donde quieras y abre `JASE.exe`.
- **`JASE-1.0.0-x86_64.AppImage`: Linux (experimental).** Hazlo ejecutable (`chmod +x`) y ábrelo. Sirve para buscar servidores, unirse a juegos de Steam y abrir los demás juegos con Wine (configurable en *Ajustes → General*; experimental). La configuración se guarda en `~/.config/jimmys-ase`.

En ambos casos la configuración se guarda en `%APPDATA%\JimmysASE` (no se escribe nada junto al exe) y desinstalar no la borra.

Ni el instalador ni el exe están firmados, así que Windows SmartScreen puede mostrar un aviso la primera vez. Pulsa **Más información → Ejecutar de todas formas**.

## Sin verificar aún en Windows

Estos puntos aún no se han verificado en una instalación real de Windows:

- Ejecutables y argumentos de lanzamiento de JKA (Jedi Academy), SoF2, BF1942 (incluido el argumento `+game` de los mods), CS 1.6, y Jedi Outcast, Xonotic, Warsow, Warfork, Tremulous y Smokin' Guns (nombres de ejecutable y `+connect`), y D-Day (`+set game dday` con `quake2.exe`, `q2pro.exe`, `r1q2.exe` o `yquake2.exe`).
- Claves de registro y carpetas de Steam que usa la búsqueda automática de juegos.
- Iconos de los ejecutables.
- Extracción de levelshots desde los archivos .pk3.
- Nuevo en la 1.0: lanzar Zandronum (`-iwad … -connect`), buscar en la red local (difusión a través del firewall de Windows), la consola RCON contra servidores reales y la actualización del zip portátil (el script `.cmd` que reemplaza la carpeta).

## Cómo reportar problemas

Abre un issue en https://github.com/JotaJazz/JASE/issues e indica el juego, qué hiciste y qué pasó.

## Créditos y licencias

- IP to Country Lite de DB-IP, CC BY 4.0.
- Banderas: flag-icons de Panayiotis Lipiridis, MIT.
- Hecho con PySide6 (Qt for Python, LGPL-3.0), keyring y Pillow.
- JASE es MIT. Los textos completos (con los avisos de terceros en `THIRD-PARTY.txt`) están en la carpeta `licenses` (dentro del zip o en la carpeta de instalación).
