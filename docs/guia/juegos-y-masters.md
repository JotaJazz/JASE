**Español** | [English](games-and-masters.md)

# Juegos y masters

Qué masters usa JASE para cada juego, cómo se verificaron y notas por juego. Para usar la app, la [guía de uso](guia-usuario.md).

## Estado de verificación

Masters verificados el 2026-10-18/19, y la API web de Steam y los masters de UT2004 el 2026-10-20; detalle en [Masters usados](#masters-usados).
Half-Life Deathmatch, Day of Defeat y Team Fortress Classic se listan por la API web de Steam (sondeo del 2026-10-20), y también Garry's Mod (sondeo del 2026-10-25: 1064 servidores no vacíos).
Los ejecutables y argumentos de conexión (`+connect`) de Jedi Academy, SoF2, CS 1.6, BF1942, Jedi Outcast, Xonotic, Warsow, Warfork, Tremulous y Smokin' Guns, y los de D-Day (`+set game dday`) y la conexión de Tactical Ops (`host:port?password=`), están pendientes de verificar en Windows.

## Masters usados

| Juego | Master | Protocolos / versiones |
|---|---|---|
| Call of Duty 2 | `cod2master.activision.com:20710`, `master.cod2x.me:20710` (el de CoD2x) | 115 = 1.0, 117 = 1.2, 118 = 1.3, 119/120 = CoD2x 1.4.x (para unirse hace falta el cliente CoD2x) |
| Call of Duty | `codmaster.activision.com:20510` | 1 = 1.1, 2 = 1.2, 4 = 1.3, 5 = 1.4, 6 = 1.5 |
| Unreal Tournament | `master.333networks.com:28900`, `master.openspy.net:28900` | 436, 451, 469 |
| Tactical Ops | `master.333networks.com:28900`, `master.openspy.net:28900` (la lista de UT) | 3.4, 3.5 (según `gametype`: `TO340`, `TO350`) |
| Call of Duty: United Offensive | `coduomaster.activision.com:20610` | 21 = 1.41, 22 = 1.51, 24 = 1.51 |
| Call of Duty 4 | `cod4master.activision.com:20810` | 6 = 1.7 (y CoD4X, versión «-»), 7 = 1.8, 5 = 1.6, 1 y 8 = 1.0 |
| Quake III Arena | `master.ioquake3.org:27950`, `master.quake3arena.com:27950`, `master.maverickservers.com:27950`, `dpmaster.deathmask.net:27950` | 68 = 1.32 (Q3 1.32e, ioq3 1.36, CNQ3 1.53) |
| Return to Castle Wolfenstein | `wolfmaster.idsoftware.com:27950` | 57, 60 = 1.41 (Wolf 1.0, Wolf 1.41/1.41b, iortcw 1.51) |
| Wolfenstein: Enemy Territory | `etmaster.idsoftware.com:27950`, `master.etlegacy.com:27950` | 84 = 2.60b / ET: Legacy |
| Medal of Honor: Allied Assault | `master.333networks.com:28900`, `master.openspy.net:28900` (gamename `mohaa`) | 1.11 (OpenMoHAA: 1.12+0.8x) |
| Medal of Honor: Spearhead | ídem (gamename `mohaas`) | 2.15 (OpenMoHAA: 2.16+0.8x) |
| Medal of Honor: Breakthrough | ídem (gamename `mohaab`) | 2.40 (OpenMoHAA: 2.41+0.8x) |
| Jedi Academy | `master.jkhub.org:29060` | 26 = 1.01 |
| Soldier of Fortune II | `master.1fxmod.org:20110` | 2002 = 1.00, 2004 = 1.03 |
| Urban Terror | `master.urbanterror.info:27900` | 68 (ioQ3 1.35 y mods; gamename `q3urt43`) |
| OpenArena | `dpmaster.deathmask.net:27950`, `master.ioquake3.org:27950` | 71 (ioq3+oa 1.36; gamename `baseoa`) |
| Jedi Outcast | `master.jkhub.org:28060`, `master.ouned.de:28060` | 15 = 1.02, 16 = 1.04 (casi todo JK2MV, `JK2MP: v1.04mv`) |
| Xonotic | `dpmaster.deathmask.net:27950`, `dpmaster.tchr.no:27950` (gamename `Xonotic`) | 3 (`gameversion` 806 = 0.8.6) |
| Warsow | `dpmaster.deathmask.net:27950`, `dpmaster.tchr.no:27950` (gamename `Warsow`) | 22 = 2.10 |
| Warfork | `dpmaster.deathmask.net:27950` (gamename `Warfork`) | 26 = 2.15 |
| Tremulous | `master.tremulous.net:30710` | 69 = 1.1 (y servidores multiprotocolo de GrangerHub 1.3) |
| Smokin' Guns | `master.smokin-guns.org:27950` | 68 (1.1, 1.2dev) |
| Counter-Strike 1.6 | `ms.cs16.net:27010` (filtro `\gamedir\cstrike`, no-steam) y, con clave de la API web de Steam, `api.steampowered.com` (`\appid\10\gamedir\cstrike`) | GoldSrc (A2S_INFO `I` y `m`), sin lista fija de versiones |
| Battlefield 1942 | `master.openspy.net:28900` (gamename `bfield1942`) | v1.61, v1.612 |
| Quake II | `master.quakeservers.net:27900` | sin lista fija de versiones (gamename `baseq2` y mods) |
| D-Day: Normandy | `master.quakeservers.net:27900` (servidores con `gamedir` `dday`) | `dll_version` del mod (5.067…) |
| Unreal Tournament 2004 | `utmaster.openspy.net:28902`, `ut2004master.333networks.com:28902` | sin lista fija de versiones (regla `ServerVersion`: 3369, 3374…) |

CoD, CoD2 y UT verificados el 2026-10-18; el resto (incluidos Jedi Academy, SoF2, CS 1.6 y BF1942) el 2026-10-19 (sondeo de cada master por protocolo y ≤ 30 consultas de estado por juego: `getstatus`, `\status\`, A2S). Jedi Outcast, Xonotic, Warsow, Warfork, Tremulous y Smokin' Guns se sondearon el 2026-10-24 (10 consultas `getstatus` por juego). D-Day: Normandy se sondeó el 2026-10-25: el master de Quake II lista todos los servidores de Quake II y JASE se queda con los de `gamedir` `dday` (1 de 95 que respondieron), que Quake II ya no muestra; se lanza como `quake2.exe +set game dday +connect ip:puerto`. dpmaster mezcla juegos, así que JASE le pide el gamename del juego (`getservers Xonotic 3 full empty`). Tactical Ops se sondeó el 2026-10-30: no tiene master propio y se anuncia como `ut`, así que JASE toma la lista de UT y la separa por `gametype` (`TO<número>`, `TOST…`, `s_SWATGame`) o por la regla `tostver` (59 de los 791 que respondieron). Con instalaciones separadas en `TO340\` y `TO350\`, JASE abre la que corresponde al servidor; los `s_SWATGame` abren la primera.

Los juegos de Steam (L4D, L4D2, CS: Source, TF2, HL DM, DoD, TFC, Garry's Mod) no tienen master: se listan por la API web de Steam (`api.steampowered.com`, requiere clave; sondeo del 2026-10-20), que la pestaña Masters muestra como su fuente. La API entrega como mucho 10 000 servidores por consulta y algunas IPs publican cientos de servidores falsos (CS 1.6, 2026-10-29: unas 200 IPs con 200 «This is a dummy CS 1.6 server!» o nombres copiados cada una), así que JASE descarta las IPs con 100 servidores o más y, mientras la respuesta llegue cortada en el tope, vuelve a consultar sin ellas.

## Notas técnicas

### Medal of Honor y OpenMoHAA

Indica la carpeta del juego: la que contiene `MOHAA.exe` (Spearhead: `moh_spearhead.exe`; Breakthrough: `moh_breakthrough.exe`) o, con OpenMoHAA, la que contiene sus lanzadores. Si en esa carpeta está el lanzador de OpenMoHAA de la variante (`launch_openmohaa_base.exe` para AA, `launch_openmohaa_spearhead.exe` para Spearhead y `launch_openmohaa_breakthrough.exe` para Breakthrough), JASE lo usa aunque la carpeta conserve `MOHAA.exe`: elegir una carpeta de OpenMoHAA es elegir OpenMoHAA. Sin lanzador, se usa el exe original; si tampoco está, en AA se prueba `openmohaa.exe`. Ajustes muestra el ejecutable elegido (p. ej. «✓ launch_openmohaa_base.exe»). Los lanzadores pasan los argumentos a `openmohaa.exe` añadiendo `+set com_target_game N`.

MOHAA se lanza como los juegos Quake 3: `+password <clave> +connect ip:puerto`. El motor registra el comando `connect` y el cvar `password`, y los argumentos `+…` se ejecutan al arrancar (`Com_AddStartupCommands`; ver `code/client/cl_main.cpp` y `code/Launcher/launch_main.cpp` en github.com/openmoh/openmohaa).

Notas del sondeo (2026-10-19, 18 consultas `\status\` por variante):

- El master lista el puerto de consulta (12300…); el `\status\` trae `hostport` con el puerto de juego, normalmente consulta − 97 (12203), que es lo que se usa si falta.
- Los tres masters mezclan variantes (p. ej. servidores de AA en la lista de Spearhead), así que se descartan los servidores cuyo `gamename` no coincide.
- MOHAA no anuncia contraseña en `\status\`: no aparece el candado y JASE no la pide. En un servidor con clave, escríbela en la consola del juego (`password <clave>` y luego `reconnect`).
- Imágenes de GameTracker: carpetas `mohaa`, `sh` y `bt`, con `/` del nombre del mapa cambiada por `_` (`obj/obj_team2` → `obj_obj_team2`). No hay extracción local de imágenes para MOHAA.

### Unreal Tournament 2004

Los masters de UT2004 (sondeo del 2026-10-20, unos 600 servidores cada uno) hablan el protocolo TCP de Epic, tal como lo implementa el servicio `utmaster` de OpenSpy (github.com/openspy/openspy-core). El cliente responde al challenge con un hash de CD key fijo y ficticio (todo ceros); nunca usa ni deriva una CD key real, y si un master lo rechaza JASE no intenta nada más. Los servidores se consultan por UDP en el puerto de consulta (puerto de juego + 1): información, reglas y jugadores. El candado de contraseña sale de la regla `GamePassword` y la columna Versión de `ServerVersion`. Se lanza como `UT2004.exe ip:puerto?password=<clave>`. Las imágenes de mapas vienen de GameTracker (carpeta `ut2k4`); no se extraen de los mapas `.ut2`.

### Zandronum (Doom)

El master de Zandronum (sondeo del 2026-10-27, unos 780 servidores) y sus servidores hablan el protocolo de lanzadores de Skulltag/Zandronum, comprimido con su árbol de Huffman fijo (portado de `huffman.cpp` de Zandronum, MIT, Timothy Landers 2009). JASE muestra nombre, mapa, modo de juego, jugadores (bots aparte), contraseña y el IWAD como *mod* en el árbol de juegos. Se lanza como `zandronum.exe -iwad <IWAD> -connect ip:puerto` (con `+cl_password`/`+cl_joinpassword` si hay contraseña). El IWAD y los PWAD del servidor tienen que estar ya en la carpeta de Zandronum: JASE no los descarga (Doomseeker sí, con Wadseeker).

### Columna Versión

En Quake III, RtCW y ET la columna Versión muestra el cvar `version` abreviado (`ioq3 1.36+u2022…/Debian linux…` → `ioq3 1.36`); en los Call of Duty, `shortversion`.

### Descartados en el sondeo

No devuelven servidores o no sirven al cliente estándar:

- Quake III: protocolo 67 (1.31, 2–4 servidores) y 71 (ioq3 nuevo, casi todo OpenArena).
- RtCW: protocolos 50 y 59 (15 servidores cada uno, casi ninguno respondió).
- ET: protocolo 83 (el master solo devuelve una entrada vacía 0.0.0.0:0) y 82 (servidores «ET 3.00», 14); `master.etlegacy.com` no responde a 82/83.
- CoD4: protocolos 2, 3 y 4 (sin servidores).
- **Counter-Strike 1.6** (protocolo Valve, sondeo del 2026-10-19): entra por `ms.cs16.net:27010` con el filtro `\gamedir\cstrike` (el `\appid\10` devuelve 0 en ese master); el A2S_INFO confirma la carpeta del juego. Los masters de Valve `hl1master.steampowered.com` y `hl2master.steampowered.com` siguen sin DNS (son CNAME a `*.discovery.steamserver.net`, NXDOMAIN en Google y Cloudflare), las IPs antiguas de los masters de Valve (puerto 27011) y `master.hlsw.org:27010/27011` no responden, y `master.csmetahook.com` ignora los filtros y corta en la página 2. Se lanza como `hl.exe -game cstrike +password <clave> +connect ip:puerto`.
- **Jedi Academy:** `masterjk3.ravensoft.com` resuelve pero no responde y `master.jk3.fun` no tiene DNS; el protocolo 25 (1.00) solo tiene 2 servidores.
- **SoF2:** `sof2master.ravensoft.com`, `master.sof2.org` y `master.sof2.net` no tienen DNS; el protocolo 2003 no tiene servidores.
- **Jedi Outcast** (2026-10-24): `masterjk2.ravensoft.com:28060` resuelve pero no responde.
- **Xonotic, Warsow, Warfork** (2026-10-24): `ghdigital.com:27950` no responde; `dpmaster.tchr.no` no devuelve servidores de Warfork.
- **World of Padman** (2026-10-24): `master.worldofpadman.net:27955` lista un solo servidor (protocolo 71) y dpmaster ninguno (gamename `WorldofPadman`), así que quedó fuera.
- **Tremulous:** los protocolos 70 y 71 no tienen servidores en `master.tremulous.net`.
- **Battlefield 1942:** `master.bf1942.org` devuelve una lista cifrada (enctype distinto de 0) y `master.333networks.com` una lista vacía.
- **QuakeWorld** (2026-10-27): los masters UDP (`master.quakeworld.nu`, `qwmaster.fodquake.net`, `master.quakeservers.net` y otros, puerto 27000) no responden y no se pudo llegar a `hubapi.quakeworld.nu`.
- **Halo: Custom Edition** (2026-10-27): `s1.master.hosthpc.com:28910` habla GameSpy 2 con el cifrado enctypeX, cuya única implementación conocida (gslist) es GPL, incompatible con la licencia MIT de JASE; los masters GameSpy clásicos (333networks, OpenSpy) no listan servidores `halom`.

