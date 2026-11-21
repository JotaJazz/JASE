# JASE 2.1.0

**Jimmy's All Seeing Eye** is a game server browser in the spirit of the classic All-Seeing Eye. It queries the master servers, lists servers with name, map, players and ping, and launches the game already connected with a double click.

## What's new in 2.1.0

**Menus and toolbar**
- Eight menus: File, View, Filter, Server, Player, Friends, Tools and Help. *Settings* is now in *Tools*.
- A new toolbar with colored icons, and Favorites, Recent and Friends at the top of the game tree.
- *Settings → Toolbar*: choose which buttons are shown, reorder them, add separators or go back to the default.

**Settings**
- *Games*: the games are listed with their icons, split into *Installed* and *Not installed*, with the details of the selected one beside the list.
- *Alerts*: turn the friend notices, the background check, the match alert and the taskbar flash on or off, and remove match thresholds.
- *Network*: an HTTP or SOCKS proxy (`http://`, `https://`, `socks4://`, `socks5://`) for everything that goes over HTTP (master lists, Steam, updates), plus timeout, retries and simultaneous queries for list scans. Game server queries (UDP) always go direct.

**Help and About**
- *Help* menu: user guide (F1), license, report a problem and website.
- *About* lists the credits of the libraries and data used, with a button for the third-party licenses.

Your settings from 2.0.0 and 1.3.0 are read as they are; the new options are only saved when you change them.

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

The Steam-listed games (L4D, L4D2, CS: Source, TF2, HL DM, DoD, TFC, Garry's Mod) need a free Steam Web API key (https://steamcommunity.com/dev/apikey), pasted in *Tools → Settings → General*. It is stored in the Windows Credential Manager.

## Downloads

- **`JASE-2.1.0-setup.exe` — installer (recommended).** Adds a Start menu shortcut (desktop shortcut optional) and an uninstaller in *Settings → Apps*. It installs just for you without admin rights, or for all users if you choose. Installed over 2.0.0 or 1.3.0, it replaces it in the same folder.
- **`JASE-2.1.0-win64.zip` — portable.** No install: unzip it anywhere and open `JASE.exe`.
- **`JASE-2.1.0-x86_64.AppImage` — Linux (experimental).** Make it executable (`chmod +x`) and run it. It browses servers, joins Steam games and opens the other games with Wine (configurable in *Settings → General*; experimental). Settings are stored in `~/.config/jimmys-ase`.

Either way, your settings are stored in `%APPDATA%\JimmysASE` (nothing is written next to the exe), and uninstalling keeps them.

Neither the installer nor the exe is code-signed, so Windows SmartScreen may warn you the first time. Click **More info → Run anyway**.

## Reporting problems

Please open an issue at https://github.com/JotaJazz/JASE/issues and include the game, what you did and what happened.

## Credits and licenses

- IP to Country Lite by DB-IP, CC BY 4.0.
- Flags: flag-icons by Panayiotis Lipiridis, MIT.
- Built with Iced and other open-source Rust libraries (listed in `THIRD-PARTY.txt`).
- JASE is MIT licensed. The full texts (including third-party notices in `THIRD-PARTY.txt`) are in the `licenses` folder (inside the zip, or in the install folder).

<!-- ES -->

# JASE 2.1.0

**Jimmy's All Seeing Eye** es un navegador de servidores de juego al estilo del clásico All-Seeing Eye. Consulta los servidores maestros, lista los servidores con nombre, mapa, jugadores y ping, y lanza el juego ya conectado con doble clic.

## Novedades de la 2.1.0

**Menús y barra de herramientas**
- Ocho menús: Archivo, Ver, Filtro, Servidor, Jugador, Amigos, Herramientas y Ayuda. *Ajustes* ahora está en *Herramientas*.
- Una barra nueva con íconos de color, y Favoritos, Recientes y Amigos arriba del árbol de juegos.
- *Ajustes → Barra*: elige qué botones se ven, ordénalos, agrega separadores o vuelve a la de fábrica.

**Ajustes**
- *Juegos*: los juegos aparecen con su ícono, separados en *Instalados* y *No instalados*, con la ficha del elegido al lado.
- *Avisos*: enciende o apaga el aviso de amigos, la revisión de fondo, el aviso de partida y el parpadeo de la barra de tareas, y quita umbrales de partida.
- *Red*: un proxy HTTP o SOCKS (`http://`, `https://`, `socks4://`, `socks5://`) para todo lo que va por HTTP (listas de masters, Steam, actualizaciones), y espera, reintentos y consultas simultáneas del escaneo de listas. Las consultas a los servidores de juego (UDP) van siempre directas.

**Ayuda y Acerca de**
- Menú *Ayuda*: guía de uso (F1), licencia, reportar un problema y sitio web.
- *Acerca de* lista los créditos de las bibliotecas y datos que se usan, con un botón para las licencias de terceros.

La configuración de la 2.0.0 y la 1.3.0 se lee tal cual; las opciones nuevas solo se guardan si las cambias.

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

Los juegos listados por Steam (L4D, L4D2, CS: Source, TF2, HL DM, DoD, TFC, Garry's Mod) necesitan una clave gratuita de la API web de Steam (https://steamcommunity.com/dev/apikey), que se pega en *Herramientas → Ajustes → General*. Se guarda en el Administrador de credenciales de Windows.

## Descargas

- **`JASE-2.1.0-setup.exe`: instalador (recomendado).** Crea un acceso directo en el menú Inicio (el del escritorio es opcional) y un desinstalador en *Configuración → Aplicaciones*. Se instala solo para ti sin permisos de administrador, o para todos los usuarios si lo eliges. Instalado sobre la 2.0.0 o la 1.3.0, la reemplaza en la misma carpeta.
- **`JASE-2.1.0-win64.zip`: portátil.** Sin instalación: descomprímelo donde quieras y abre `JASE.exe`.
- **`JASE-2.1.0-x86_64.AppImage`: Linux (experimental).** Hazlo ejecutable (`chmod +x`) y ábrelo. Sirve para buscar servidores, unirse a juegos de Steam y abrir los demás juegos con Wine (configurable en *Ajustes → General*; experimental). La configuración se guarda en `~/.config/jimmys-ase`.

En ambos casos la configuración se guarda en `%APPDATA%\JimmysASE` (no se escribe nada junto al exe) y desinstalar no la borra.

Ni el instalador ni el exe están firmados, así que Windows SmartScreen puede mostrar un aviso la primera vez. Pulsa **Más información → Ejecutar de todas formas**.

## Cómo reportar problemas

Abre un issue en https://github.com/JotaJazz/JASE/issues e indica el juego, qué hiciste y qué pasó.

## Créditos y licencias

- IP to Country Lite de DB-IP, CC BY 4.0.
- Banderas: flag-icons de Panayiotis Lipiridis, MIT.
- Hecho con Iced y otras bibliotecas libres de Rust (listadas en `THIRD-PARTY.txt`).
- JASE es MIT. Los textos completos (con los avisos de terceros en `THIRD-PARTY.txt`) están en la carpeta `licenses` (dentro del zip o en la carpeta de instalación).
