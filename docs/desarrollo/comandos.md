# Comandos

Entorno: `python -m venv .venv && .venv/bin/pip install -e ".[dev]"` (en Windows,
`.venv\Scripts\`). Tras subir la versión en `pyproject.toml`:
`.venv/bin/pip install -e . --no-deps` (si no, `importlib.metadata` sigue viendo la vieja).

## Calidad (lo mismo que el CI)

| Qué | Comando | Debe quedar |
|---|---|---|
| Tests | `.venv/bin/pytest -q -W error` | todo verde y sin avisos (Qt offscreen vía `tests/conftest.py`) |
| Lint | `.venv/bin/ruff check src tests scripts` | sin errores |
| Tipos | `.venv/bin/mypy` | sin errores |
| Documentación | `.venv/bin/python scripts/check_docs.py` | sin salida (también corre en los tests) |

## Rust (rama `rust`, desde `rust/`)

Port en curso ([diseño](../hoja-de-ruta/port-rust.md)); lo mismo que el job `rust` del CI.

| Qué | Comando | Debe quedar |
|---|---|---|
| Formato | `cargo fmt --check` | sin diferencias |
| Lint | `cargo clippy --all-targets -- -D warnings` | sin avisos |
| Tests | `cargo test` | todo verde (incluye la paridad con Python y `proptest`) |
| Imágenes de mapas | `cargo clippy -p jase-core --all-targets --features map-images -- -D warnings` y `cargo test -p jase-core --features map-images` | sin avisos y verde (IWI, DXT contra Pillow, capturas de `.unr`, servicio de imágenes con su caché; la feature `map-images` está apagada por defecto) |
| Golden de paridad | `.venv/bin/python scripts/protocol_golden.py` (desde la raíz) | regenera `rust/jase-core/tests/golden/`; solo si cambia un fixture (Python está congelado en 1.3.0) |
| Golden de juegos y datos | `.venv/bin/python scripts/core_golden.py` | regenera `rust/jase-core/tests/golden/games.json`, `countries.json` (nombres de países por idioma) y los de `rust/jase-core/tests/golden/data/` (config, caché, historial, exportación); solo si cambia `src/jase/games.py` o el formato de esos archivos (Python está congelado en 1.3.0) |
| Tabla de juegos de Rust | `.venv/bin/python scripts/gen_games_rs.py` | regenera `rust/jase-core/src/games_table.rs` desde `games.py`; mientras Python siga en el repo, la tabla no se edita a mano |
| Catálogos de traducción | `.venv/bin/python scripts/export_locales.py` | regenera `rust/jase-core/locales/*.json` (y `countries.json`, los nombres de países) desde `src/jase/locales/` |
| Paleta de temas de Rust | `QT_QPA_PLATFORM=offscreen .venv/bin/python scripts/gen_palette_rs.py` | regenera `rust/jase/src/theme/palette.rs` desde `src/jase/ui/theme.py`; no se edita a mano (lo verifica `cargo test -p jase --test theme`) |
| Paridad de `jase-cli` | `cargo test -p jase-cli --test parity_python -- --nocapture` | verde; compara con `.venv/bin/jase-cli` (sin `.venv` avisa y se salta) |
| `jase-cli` de Rust | `cargo run -q -p jase-cli -- scan cod2 --json` | mismos comandos y JSON que [la CLI](../cli.md) de Python |
| App de Rust | `cargo run -p jase` (`--release` para probar 5.000+ filas) | abre la ventana; en WSL fuerza X11 sola (sin pantalla: `ICED_BACKEND=tiny-skia` y `xvfb-run -a`) |
| Autocomprobación de la app | `cargo run -p jase -- --smoke` | imprime `JASE smoke OK <versión>` y sale con 0 (abre la ventana con 5.000 servidores falsos, sin red ni la config del usuario); la corre `cargo test -p jase --test smoke`, que sin `DISPLAY` ni `WAYLAND_DISPLAY` se salta (en el CI de Linux va con `xvfb-run -a -s "-screen 0 1920x1080x24" cargo test`) |
| Capturas de la app de Rust | `ICED_BACKEND=tiny-skia cargo run --release -p jase -- --screenshot <carpeta>` | escribe `<tema>.png` de los 14 temas , `<tema>-password.png` con el diálogo de contraseña del servidor elegido abierto y `<tema>-add.png` con «Agregar servidor» abierto con `connect 192.0.2.14:28960` (1280×800 × la escala de la pantalla; en Windows, recortadas si el escritorio es más chico, como el de 1024×768 del CI; inglés, los servidores de ejemplo de `scripts/screenshot.py` con el árbol de CoD2 abierto, la barra lateral de 300 px y *País* a 70 px, sin red, la config ni el llavero del usuario) e imprime `JASE screenshot OK 42`; lo prueban `cargo test -p jase --test screenshot` y `--lib screenshot` |

## Ejecutar

| Qué | Comando |
|---|---|
| App (WSLg en Linux) | `.venv/bin/python -m jase` — para desarrollo: «Conectar» no lanza juegos de Windows desde WSL (Wine es experimental); la app real corre en Windows |
| Prueba de arranque del empaquetado | `.venv/bin/python -m jase --smoke` |
| CLI | `.venv/bin/jase-cli games` · `.venv/bin/jase-cli scan <id> --json` (contrato: [docs/cli.md](../cli.md)) |

## Sondeos (usan la red)

| Qué | Comando | Resultado esperado |
|---|---|---|
| Masters | `.venv/bin/python scripts/probe_masters.py <ids> --sample 10` | servidores listados y respuestas (sin `--sample`, todos) |
| «Detectar juego» | `.venv/bin/python scripts/probe_detect.py [ids] [--sample 10]` | todo `ok` tras tocar `src/jase/identify.py`; Steam necesita `JASE_STEAM_KEY` |
| Imágenes de mapas | `.venv/bin/python scripts/probe_map_images.py [ids]` | 200 en todos (carpetas y nombres de GameTracker) |

## Recursos

| Qué | Comando |
|---|---|
| Icono de la app (.ico) | `.venv/bin/python scripts/make_icon.py` tras editar los SVG |
| Iconos del tema Clásico | `.venv/bin/python scripts/make_win98_icons.py [--preview RUTA]` (pixel art en el script; los de XP son SVG en `src/jase/assets/icons/xp/`) |
| Capturas del README | `.venv/bin/python scripts/screenshot.py --lang en` y `--lang es` |
| Capturas de Python para comparar con Rust | `for v in dark light neon brown green classic xp synthwave rust goldsrc; do .venv/bin/python scripts/screenshot.py --lang en --theme $v --out <carpeta>/$v.png; done` y `--theme terminal --terminal-color {green,white,sepia,amber}` → `<carpeta>/terminal_<color>.png` (los mismos nombres que `--screenshot` de Rust); con `--password-dialog --out <carpeta>/<tema>-password.png`, solo el diálogo de contraseña (guardada «secreto»); con `--add-server-dialog --out <carpeta>/<tema>-add.png`, solo «Agregar servidor» con `connect 192.0.2.14:28960` |
| Nombres de países | `.venv/bin/python scripts/gen_countries_en.py` (Qt) · `python scripts/gen_countries.py pt fr de it` (necesita `babel`) |
| Avisos de terceros | `python scripts/collect_licenses.py <salida>` (lo usa el Release) |
| AppImage | `scripts/build_appimage.sh <dist/JASE> <versión> <appimagetool> <runtime>` (lo usa el Release) |
