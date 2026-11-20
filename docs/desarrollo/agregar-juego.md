# Agregar un juego

Requisito ([ADR 0005](../arquitectura/adr/0005-juegos-con-sondeo-real.md)): su master
devuelve servidores en un sondeo real y los servidores responden. Referencias: Tactical
Ops (juego en una familia existente, commit df8750b) y Zandronum (familia nueva,
commit f27b476).

## Juego en una familia existente

| Paso | Dónde |
|---|---|
| Sondeo del master y de una muestra de servidores; guardar respuestas reales | sondeo con `cargo run -q -p jase-cli -- scan <id> --master HOST:PUERTO` (desde `rust/`); las respuestas binarias se capturan a mano (p. ej. Wireshark o `tcpdump`, no hay herramienta en el repo) en `tests/fixtures/<juego>_*.bin` (binarios por `.gitattributes`) |
| `GameDef` (familia, masters, exe, `launch_style`, filtros del master), con un comentario de lo sondeado | `rust/jase-core/src/games_table.rs` |
| Mismos masters en los dos lugares | `masters.json` (subir `updated`) y `rust/jase-core/src/games_table.rs` |
| Badge del juego en todos los temas | `rust/jase/src/theme/palette.rs` (campo `badge_<id>` en cada `Palette` y su rama en `Palette::badge`) |
| Puerto de consulta para LAN | `rust/jase-core/src/lan.rs` (`GAME_PORTS`) |
| Detección de instalados (opcional) | `rust/jase-core/src/detect.rs` (registro, Steam, carpetas) |
| Tests contra las capturas | `rust/jase-core/tests/` (el de su familia, p. ej. `quake3.rs`, y `games.rs`) |
| Tabla de juegos | `README.md` y `README.es.md` |
| Exe y carpeta del juego | `docs/guia/user-guide.md` y `docs/guia/guia-usuario.md` |
| Masters, notas y «pendiente de verificar en Windows» | `docs/guia/games-and-masters.md` y `docs/guia/juegos-y-masters.md` |
| Notas de la versión | `packaging/release-notes.md` |

## Familia nueva

Además de lo anterior:

| Paso | Dónde |
|---|---|
| Parser puro (bytes → datos, `ProtocolError` si algo no cuadra; sin pánicos) | `rust/jase-core/src/protocols/` (un archivo por familia) y su caso en `rust/jase-core/tests/robustness.rs` |
| `Family` y, si hace falta, `LaunchStyle` | `rust/jase-core/src/games.rs` |
| Master y consulta | `rust/jase-core/src/scanner/` (`scan::fetch_master`, `query::query_one`) |
| Difusión LAN | `rust/jase-core/src/lan.rs` (`FAMILY_PORTS`, `probe`) |
| «Detectar juego» | `rust/jase-core/src/identify.rs`; luego `jase-cli scan` contra servidores reales |
| Argumentos de lanzamiento validados | `rust/jase-core/src/launcher.rs` |
| Licencia de código de terceros, si se adapta | `packaging/licenses/` y los scripts de empaquetado que la copian; un crate nuevo, en `rust/about.toml` si su licencia no está aceptada |

Los argumentos de lanzamiento no se pueden probar desde Linux: anotarlos como pendientes
de verificar en Windows ([deuda](../hoja-de-ruta/deuda.md)).
