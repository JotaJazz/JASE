# JASE 2.0.0

**Jimmy's All Seeing Eye** is a game server browser in the spirit of the classic All-Seeing Eye. It queries the master servers, lists servers with name, map, players and ping, and launches the game already connected with a double click.

## What's new in 2.0.0

JASE has been rewritten in Rust. It looks and works like 1.3.0, but it no longer carries Python and Qt, so the download is smaller.

**Nothing changes for you**
- Your settings, favorites, saved server lists and passwords stay where they were and are read as they are.
- JASE 1.3.0 offers this version in *Help → Check for updates…* and installs it, both the installed and the portable one.
- To go back to 1.3.0, reinstall `JASE-1.3.0-setup.exe` (or unzip `JASE-1.3.0-win64.zip`) from the 1.3.0 release: it reads the same settings. While 2.0.0 is the newest version, 1.3.0 will keep offering it; just decline.

**Known differences from 1.3.0**
- No screen reader support yet (the new interface library does not offer it).
- On Linux with Wayland, minimizing does not hide JASE to the tray and the window position is not remembered.
- Menus cannot be navigated with the arrow keys and have no underlined shortcut letters.
- Long text in the server list is cut without "…".
- Small visual details in some themes (menus, tabs, dialog buttons) differ slightly.

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

- **`JASE-2.0.0-setup.exe` — installer (recommended).** Adds a Start menu shortcut (desktop shortcut optional) and an uninstaller in *Settings → Apps*. It installs just for you without admin rights, or for all users if you choose. Installed over 1.3.0, it replaces it in the same folder.
- **`JASE-2.0.0-win64.zip` — portable.** No install: unzip it anywhere and open `JASE.exe`.
- **`JASE-2.0.0-x86_64.AppImage` — Linux (experimental).** Make it executable (`chmod +x`) and run it. It browses servers, joins Steam games and opens the other games with Wine (configurable in *Settings → General*; experimental). Settings are stored in `~/.config/jimmys-ase`.

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

# JASE 2.0.0

**Jimmy's All Seeing Eye** es un navegador de servidores de juego al estilo del clásico All-Seeing Eye. Consulta los servidores maestros, lista los servidores con nombre, mapa, jugadores y ping, y lanza el juego ya conectado con doble clic.

## Novedades de la 2.0.0

JASE se reescribió en Rust. Se ve y funciona como la 1.3.0, pero ya no lleva Python ni Qt, así que la descarga es más chica.

**Para ti no cambia nada**
- La configuración, los favoritos, las listas de servidores guardadas y las contraseñas siguen en el mismo lugar y se leen tal cual.
- JASE 1.3.0 ofrece esta versión en *Ayuda → Buscar actualizaciones…* y la instala, tanto la instalada como la portátil.
- Para volver a la 1.3.0, reinstala `JASE-1.3.0-setup.exe` (o descomprime `JASE-1.3.0-win64.zip`) de la release 1.3.0: lee la misma configuración. Mientras la 2.0.0 sea la más nueva, la 1.3.0 la seguirá ofreciendo; basta con decir que no.

**Diferencias conocidas con la 1.3.0**
- Todavía sin soporte para lectores de pantalla (la nueva biblioteca de interfaz no lo ofrece).
- En Linux con Wayland, minimizar no oculta JASE en la bandeja y no se recuerda la posición de la ventana.
- Los menús no se recorren con las flechas ni tienen letras subrayadas de acceso.
- El texto largo de la lista de servidores se corta sin «…».
- Algunos detalles visuales de ciertos temas (menús, pestañas, botones de los diálogos) cambian un poco.

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

- **`JASE-2.0.0-setup.exe`: instalador (recomendado).** Crea un acceso directo en el menú Inicio (el del escritorio es opcional) y un desinstalador en *Configuración → Aplicaciones*. Se instala solo para ti sin permisos de administrador, o para todos los usuarios si lo eliges. Instalado sobre la 1.3.0, la reemplaza en la misma carpeta.
- **`JASE-2.0.0-win64.zip`: portátil.** Sin instalación: descomprímelo donde quieras y abre `JASE.exe`.
- **`JASE-2.0.0-x86_64.AppImage`: Linux (experimental).** Hazlo ejecutable (`chmod +x`) y ábrelo. Sirve para buscar servidores, unirse a juegos de Steam y abrir los demás juegos con Wine (configurable en *Ajustes → General*; experimental). La configuración se guarda en `~/.config/jimmys-ase`.

En ambos casos la configuración se guarda en `%APPDATA%\JimmysASE` (no se escribe nada junto al exe) y desinstalar no la borra.

Ni el instalador ni el exe están firmados, así que Windows SmartScreen puede mostrar un aviso la primera vez. Pulsa **Más información → Ejecutar de todas formas**.

## Cómo reportar problemas

Abre un issue en https://github.com/JotaJazz/JASE/issues e indica el juego, qué hiciste y qué pasó.

## Créditos y licencias

- IP to Country Lite de DB-IP, CC BY 4.0.
- Banderas: flag-icons de Panayiotis Lipiridis, MIT.
- Hecho con Iced y otras bibliotecas libres de Rust (listadas en `THIRD-PARTY.txt`).
- JASE es MIT. Los textos completos (con los avisos de terceros en `THIRD-PARTY.txt`) están en la carpeta `licenses` (dentro del zip o en la carpeta de instalación).
