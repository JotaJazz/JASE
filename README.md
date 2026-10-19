# Jimmy's All Seeing Eye

Navegador de servidores de juego al estilo del clásico *All-Seeing Eye*, para
**Call of Duty 2**, **Call of Duty** y **Unreal Tournament (UT99)**. Consulta los
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

Verificados el 2026-10-18 con `scripts/probe_masters.py`.
