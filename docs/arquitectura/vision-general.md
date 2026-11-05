# Visión general

JASE es un navegador de servidores de juegos clásicos: pide la lista a los masters,
consulta cada servidor por UDP y lanza el juego conectado. Una app de escritorio
(PySide6) y una CLI sin interfaz comparten el mismo núcleo.

## Capas

```
src/jase/
├── protocols/   parsers puros por familia (bytes → ServerInfo / direcciones); sin red
├── scanner.py   asyncio: masters + consultas UDP en paralelo, una familia por llamada
├── games.py     GameDef por juego (familia, masters, exe, lanzamiento, filtros)
├── models.py    ServerAddress, ServerInfo, Player, QuickFilters
├── núcleo       config, masters, launcher, detect, identify, lan, rcon, geo, history,
│                server_cache, app_update, portable_update, i18n, …
│                paths.py: exe y carpetas anidadas (sin distinguir mayúsculas)
│                browse.py: mods, mapas y países de una lista (desglose del árbol)
│                net.py: descargas HTTP pequeñas con plazo y tope de tamaño
├── maps/        imágenes de mapas (IWI de CoD, .unr de UT99, levelshots, GameTracker)
├── cli.py       jase-cli: JSON versionado (contrato en docs/cli.md)
└── ui/          PySide6: MainWindow + mixins window_*.py, workers, cargadores, tema
```

Detalle de cada capa: [protocolos.md](protocolos.md), [ui.md](ui.md). Reglas de
dependencia: [limites.md](limites.md).

## Flujo principal: escanear un juego

1. `ui/main_window.py` (`select_game`): lista de la sesión, la guardada en disco
   (`server_cache`, en un hilo) o un escaneo nuevo.
2. `ui/scan_worker.py` (`ScanWorker`): un `QThread` con su propio bucle asyncio llama
   a `scanner.scan`.
3. `scanner.scan`: `fetch_master` por cada master (en paralelo), deduplica direcciones
   y `query_servers` las consulta por un socket UDP compartido (64 a la vez, reintento).
4. Cada respuesta válida → `protocols/<familia>.py` → `ServerInfo` → señal Qt al hilo de
   la UI → `ServerTableModel.add_or_update`.
5. Escaneo completo sin error: la lista pasa a la caché de la sesión y a
   `cache/servers/<juego>.json` (ver [ADR 0006](adr/0006-listas-guardadas.md)).
6. Doble clic → `launcher.launch`: argv según `GameDef.launch_style`, contraseña del
   llavero (`passwords.py`, keyring).

## Datos que se guardan

| Qué | Dónde | Módulo |
|---|---|---|
| Configuración (rutas, favoritos, filtros, estado de la ventana) | `%APPDATA%\JimmysASE\config.json` o `~/.config/jimmys-ase/config.json` | `config.py` |
| Contraseñas de servidor y RCON, clave de Steam | llavero del sistema (servicio `JimmysAllSeeingEye`) | `passwords.py` |
| Masters personales | `masters.custom.json` junto a la config | `masters.py` |
| Lista oficial de masters descargada | `cache/masters.json` + meta | `masters.py` |
| Listas de servidores entre sesiones | `cache/servers/` | `server_cache.py` |
| Historial de ocupación (favoritos, recientes) | `cache/history.json` | `history.py` |
| Base de países (DB-IP Lite) | `cache/dbip-country-lite.csv` | `geo.py` |
| Imágenes de mapas | `cache/maps/` | `maps/service.py` |
| Descargas de actualización | `cache/updates/` | `app_update.py`, `portable_update.py` |

Todo se escribe de forma atómica (archivo temporal + `os.replace`; casi siempre con
`fetch_meta.atomic_write`) y se carga de forma tolerante: cada campo o entrada inválida
vuelve a su valor por defecto sin tumbar el resto.

## Ejecutables

| Comando | Entrada | Necesita |
|---|---|---|
| `jase` / `python -m jase` | `src/jase/__main__.py` | extra `gui` (PySide6, keyring, Pillow) |
| `jase-cli` | `src/jase/cli.py` | solo la biblioteca estándar |

Empaquetado: PyInstaller (`jase.spec`), instalador Inno Setup (`packaging/jase.iss`),
zip portátil y AppImage (`scripts/build_appimage.sh`), en `.github/workflows/release.yml`.
