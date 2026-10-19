# Jimmy's All Seeing Eye

Navegador de servidores de juego al estilo del clásico *All-Seeing Eye*, para
**Call of Duty 2**, **Call of Duty**, **Call of Duty: United Offensive**,
**Call of Duty 4**, **Quake III Arena**, **Return to Castle Wolfenstein**,
**Wolfenstein: Enemy Territory**, **Unreal Tournament (UT99)** y **Medal of Honor:
Allied Assault** (con **Spearhead** y **Breakthrough**). Consulta los
masters, muestra los servidores con nombre, mapa, jugadores, versión y ping, y
abre el juego ya conectado con doble clic.

## Requisitos

- Windows 10/11.
- **Python 3.11 o superior para Windows** (de [python.org](https://www.python.org/downloads/windows/);
  marca «Add python.exe to PATH» y deja instalado el lanzador `py`).
- Los juegos instalados (la ruta de cada uno se configura dentro de la app, en ⚙ Ajustes).

## Instalación (PowerShell)

Desde la carpeta del proyecto:

```powershell
py -m venv .venv
.venv\Scripts\pip install -e .
```

## Ejecutar

```powershell
.venv\Scripts\python -m jase
```

(También queda disponible el comando `.venv\Scripts\jase`.)

La primera vez, abre **⚙ Ajustes** e indica la carpeta de instalación de cada
juego (la que contiene `CoD2MP_s.exe`, `CoDMP.exe` o `System\UnrealTournament.exe`).

**Medal of Honor y OpenMoHAA.** La carpeta de MOHAA es la que contiene `MOHAA.exe`
(Spearhead: `moh_spearhead.exe`; Breakthrough: `moh_breakthrough.exe`). Si esa
carpeta no tiene el exe original, JASE prueba los de OpenMoHAA en este orden:
`launch_openmohaa_base.exe` y `openmohaa.exe` (AA), `launch_openmohaa_spearhead.exe`
(Spearhead) y `launch_openmohaa_breakthrough.exe` (Breakthrough). El exe original
tiene prioridad: una carpeta de OpenMoHAA que conserve `MOHAA.exe` abre el juego
original. Para jugar con OpenMoHAA, indica una carpeta sin `MOHAA.exe`. Los
lanzadores de OpenMoHAA pasan los argumentos a `openmohaa.exe` añadiendo
`+set com_target_game N`. MOHAA se lanza como los juegos Quake 3:
`+password <clave> +connect ip:puerto`. El motor registra el comando `connect` y
el cvar `password`, y los argumentos `+…` se ejecutan al arrancar
(`Com_AddStartupCommands`; ver `code/client/cl_main.cpp` y
`code/Launcher/launch_main.cpp` en github.com/openmoh/openmohaa).
Elige el juego en la barra lateral, pulsa **⟳ Refrescar** y haz doble clic en un
servidor para entrar.

## Tests

```powershell
.venv\Scripts\pip install -e ".[dev]"
.venv\Scripts\python -m pytest
```

Para comprobar que los masters reales responden (hace unas pocas consultas
pequeñas por juego):

```powershell
.venv\Scripts\python scripts\probe_masters.py
```

## Configuración y contraseñas

- La configuración (rutas de los juegos, tema, filtro de versión y masters) se
  guarda en `%APPDATA%\JimmysASE\config.json`. Se puede editar a mano con la app
  cerrada; por ejemplo, la clave `masters` permite reemplazar los masters de un
  juego (`{"masters": {"ut99": [["master.333networks.com", 28900]]}}`).
- Las contraseñas de servidores **no** se guardan en ese archivo: se almacenan en
  el **Administrador de credenciales de Windows** (servicio `JimmysAllSeeingEye`).
  Se pueden ver o borrar en *Panel de control → Administrador de credenciales →
  Credenciales de Windows*.

## Masters usados

| Juego | Master | Protocolos / versiones |
|---|---|---|
| Call of Duty 2 | `cod2master.activision.com:20710` | 115 = 1.0, 117 = 1.2, 118 = 1.3 |
| Call of Duty | `codmaster.activision.com:20510` | 1 = 1.1, 2 = 1.2, 4 = 1.3, 5 = 1.4, 6 = 1.5 |
| Unreal Tournament | `master.333networks.com:28900`, `master.openspy.net:28900` | 436, 451, 469 |
| Call of Duty: United Offensive | `coduomaster.activision.com:20610` | 21 = 1.41, 22 = 1.51, 24 = 1.51 |
| Call of Duty 4 | `cod4master.activision.com:20810` | 6 = 1.7 (y CoD4X, versión «-»), 7 = 1.8, 5 = 1.6, 1 y 8 = 1.0 |
| Quake III Arena | `master.ioquake3.org:27950`, `master.quake3arena.com:27950`, `master.maverickservers.com:27950` | 68 = 1.32 (Q3 1.32e, ioq3 1.36, CNQ3 1.53) |
| Return to Castle Wolfenstein | `wolfmaster.idsoftware.com:27950` | 57, 60 = 1.41 (Wolf 1.0, Wolf 1.41/1.41b, iortcw 1.51) |
| Wolfenstein: Enemy Territory | `etmaster.idsoftware.com:27950`, `master.etlegacy.com:27950` | 84 = 2.60b / ET: Legacy |
| Medal of Honor: Allied Assault | `master.333networks.com:28900`, `master.openspy.net:28900` (gamename `mohaa`) | 1.11 (OpenMoHAA: 1.12+0.8x) |
| Medal of Honor: Spearhead | ídem (gamename `mohaas`) | 2.15 (OpenMoHAA: 2.16+0.8x) |
| Medal of Honor: Breakthrough | ídem (gamename `mohaab`) | 2.40 (OpenMoHAA: 2.41+0.8x) |

CoD, CoD2 y UT verificados el 2026-10-18; el resto el 2026-10-19 (sondeo de cada
master por protocolo y ≤ 30 consultas `getstatus` por juego).

Medal of Honor (2026-10-19, 18 consultas `\status\` por variante): el master lista
el puerto de consulta (12300…); el `\status\` trae `hostport` con el puerto de
juego, normalmente consulta − 97 (12203), que es lo que se usa si falta. Los tres
masters mezclan variantes (p. ej. servidores de AA en la lista de Spearhead), así
que se descartan los servidores cuyo `gamename` no coincide. MOHAA no anuncia
contraseña en `\status\`: no aparece el candado y JASE no la pide. En un
servidor con clave, escríbela en la consola del juego (`password <clave>` y luego
`reconnect`). Imágenes de GameTracker: carpetas `mohaa`, `sh` y
`bt`, con `/` del nombre del mapa cambiada por `_` (`obj/obj_team2` →
`obj_obj_team2`). No hay extracción local de imágenes para MOHAA.

En Quake III, RtCW y ET la columna Versión muestra el cvar `version` abreviado
(`ioq3 1.36+u2022…/Debian linux…` → `ioq3 1.36`); en los Call of Duty, `shortversion`.

Descartados en el sondeo (no devuelven servidores o no sirven al cliente estándar):
- Quake III: protocolo 67 (1.31, 2–4 servidores) y 71 (ioq3 nuevo, casi todo OpenArena).
- RtCW: protocolos 50 y 59 (15 servidores cada uno, casi ninguno respondió).
- ET: protocolo 83 (el master solo devuelve una entrada vacía 0.0.0.0:0) y 82
  (servidores «ET 3.00», 14); `master.etlegacy.com` no responde a 82/83.
- CoD4: protocolos 2, 3 y 4 (sin servidores).
