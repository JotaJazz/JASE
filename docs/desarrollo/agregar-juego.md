# Agregar un juego

Requisito ([ADR 0005](../arquitectura/adr/0005-juegos-con-sondeo-real.md)): su master
devuelve servidores en un sondeo real y los servidores responden. Referencias: Tactical
Ops (juego en una familia existente, commit df8750b) y Zandronum (familia nueva,
commit f27b476).

## Juego en una familia existente

| Paso | Dónde |
|---|---|
| Sondeo del master y de una muestra de servidores; guardar respuestas reales | `scripts/probe_masters.py` → `tests/fixtures/<juego>_*.bin` (binarios por `.gitattributes`) |
| `GameDef` (familia, masters, exe, `launch_style`, filtros del master) | `src/jase/games.py` |
| Mismos masters en las dos copias | `masters.json` y `src/jase/assets/masters.json` (subir `updated`) |
| Badge del juego en todos los temas | `src/jase/ui/theme.py` (`_BADGES_DARK`, `_BADGES_LIGHT`) |
| Puerto de consulta para LAN | `src/jase/lan.py` (`GAME_PORTS`) |
| Detección de instalados (opcional) | `src/jase/detect.py` (registro, Steam, carpetas) |
| Tests contra las capturas | `tests/test_games_<algo>.py` |
| Tabla de juegos | `README.md` y `README.es.md` |
| Exe y carpeta del juego | `docs/guia/user-guide.md` y `docs/guia/guia-usuario.md` |
| Masters, notas y «pendiente de verificar en Windows» | `docs/guia/games-and-masters.md` y `docs/guia/juegos-y-masters.md` |
| Notas de la versión | `packaging/release-notes.md` |

## Familia nueva

Además de lo anterior:

| Paso | Dónde |
|---|---|
| Parser puro (bytes → datos, `ProtocolError` si algo no cuadra) | `src/jase/protocols/<familia>.py` |
| `family` y, si hace falta, `launch_style` en los `Literal` | `src/jase/games.py` |
| Master y consulta | `src/jase/scanner.py` (`FAMILIES`, `fetch_master`, `_query_one`) |
| Difusión LAN | `src/jase/lan.py` (`FAMILY_PORTS`, `PROBES`) |
| «Detectar juego» | `src/jase/identify.py`; luego `scripts/probe_detect.py` todo `ok` |
| Argumentos de lanzamiento validados | `src/jase/launcher.py` |
| Licencia de código de terceros, si se adapta | `scripts/collect_licenses.py` y THIRD-PARTY |

Los argumentos de lanzamiento no se pueden probar desde Linux: anotarlos como pendientes
de verificar en Windows ([deuda](../hoja-de-ruta/deuda.md)).
