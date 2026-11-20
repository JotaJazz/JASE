# Comandos

Entorno: [Rust](https://rustup.rs) con `rustup` (la `rust-version` de `rust/Cargo.toml`); en
Windows, la cadena MSVC. Todo se corre desde `rust/` salvo los scripts de empaquetado, que van
desde la raíz.

## Calidad (lo mismo que el CI)

| Qué | Comando | Debe quedar |
|---|---|---|
| Formato | `cargo fmt --check` | sin diferencias |
| Lint | `cargo clippy --all-targets -- -D warnings` | sin avisos |
| Tests | `cargo test` | todo verde (incluye `proptest`); sin pantalla, `xvfb-run -a -s "-screen 0 1920x1080x24" cargo test` |
| Imágenes de mapas | `cargo clippy -p jase-core --all-targets --features map-images -- -D warnings` y `cargo test -p jase-core --features map-images` | sin avisos y verde (IWI, DXT con las respuestas fijas de Pillow, capturas de `.unr`, servicio de imágenes con su caché; la feature `map-images` está apagada por defecto) |
| Documentación | `cargo test -p jase --test check_docs --test parity_docs` | verde (enlaces, rutas, datos personales, etapas de la memoria y la tabla de paridad; también corre en `cargo test`) |

## Ejecutar

| Qué | Comando | Resultado |
|---|---|---|
| App | `cargo run -p jase` (`--release` para probar 5.000+ filas) | abre la ventana; en WSL fuerza X11 sola (sin pantalla: `ICED_BACKEND=tiny-skia` y `xvfb-run -a`) |
| Autocomprobación de la app | `cargo run -p jase -- --smoke` | imprime `JASE smoke OK <versión>` y sale con 0 (abre la ventana con 5.000 servidores falsos, sin red ni la config del usuario); la corre `cargo test -p jase --test smoke`, que sin `DISPLAY` ni `WAYLAND_DISPLAY` se salta (en el CI de Linux va con `xvfb-run -a -s "-screen 0 1920x1080x24" cargo test`) |
| Capturas de la app de Rust | `ICED_BACKEND=tiny-skia cargo run --release -p jase -- --screenshot <carpeta>` | escribe `<tema>.png` de los 14 temas, `<tema>-password.png` con el diálogo de contraseña del servidor elegido abierto, `<tema>-add.png` con «Agregar servidor» abierto con `connect 192.0.2.14:28960`, `<tema>-settings-{games,masters,general,appearance}.png` con Ajustes de la config por omisión (solo CoD2 instalado) en cada pestaña, `<tema>-find.png` con *Buscar juegos* y dos juegos hallados, `<tema>-friends.png` con *Amigos* y `<tema>-party.png` con el umbral de partida, los dos vigilando un servidor lleno, y del servidor elegido `<tema>-context.png` (el menú contextual), `<tema>-about.png` (*Acerca de*), `<tema>-note.png` (*Editar nota…* con «Clan amigo, martes 21 h») y `<tema>-slotfree.png` («Hay sitio») (1280×800 × la escala de la pantalla; en Windows, recortadas si el escritorio es más chico, como el de 1024×768 del CI; inglés, servidores de ejemplo fijos con el árbol de CoD2 abierto, la barra lateral de 300 px y *País* a 70 px, sin red, la config ni el llavero del usuario), y en su propia ventana `<tema>-now.png` (*Ahora se juega* con CoD2, dos servidores de CoD1 y un escaneo sin terminar, 760×480), `<tema>-search.png` (*Buscar jugador* con «jan», 640×420) y `<tema>-rcon.png` (la consola tras `status` contra una respuesta fija, 640×440), e imprime `JASE screenshot OK 238`; lo prueban `cargo test -p jase --test screenshot` y `--lib screenshot` |
| CLI | `cargo run -q -p jase-cli -- games` · `cargo run -q -p jase-cli -- scan <id> --json` | contrato: [docs/cli.md](../cli.md); instalada con `cargo install --path rust/jase-cli --locked` (desde la raíz) |

«Conectar» no lanza juegos de Windows desde WSL (Wine es experimental): la app real corre en
Windows.

## Sondeos (usan la red)

| Qué | Comando | Resultado esperado |
|---|---|---|
| Masters | `cargo run -q -p jase-cli -- scan <id>` (`--master HOST:PUERTO` para uno que no está en la lista) | servidores listados y respuestas |

## Empaquetado

| Qué | Comando |
|---|---|
| AppImage | `scripts/package_linux.sh <appimagetool> <runtime>` desde la raíz: deja `dist/JASE/` y `JASE-<v>-x86_64.AppImage` con su prueba de humo (lo usa `package.yml`; `JASE_GLIBC_MAX=` vacío quita el tope de glibc 2.35 en local) |
| AppImage desde una carpeta | `scripts/build_appimage.sh <dist/JASE> <versión> <appimagetool> <runtime>` (lo llama `package_linux.sh`) |
| Zip e instalador | `./scripts/package_windows.ps1 [-Iscc <ISCC.exe>]` (PowerShell 7, Inno Setup 6): deja `dist\JASE\`, `JASE-<v>-win64.zip` y `JASE-<v>-setup.exe` con su prueba de humo (lo usa `package.yml`) |
| Prueba de actualización desde 1.3.0 | `./scripts/test_upgrade_windows.ps1 -Version <v> [-Ci]` tras `package_windows.ps1`: instala sobre la 1.3.0 publicada y actualiza un portátil 1.3.0; sin `-Ci` no toca una JASE ya instalada (lo corre `package.yml`) |
| Avisos de terceros | `cargo about generate -c about.toml about.hbs -o <salida>/THIRD-PARTY.txt` desde `rust/` (`cargo install cargo-about --version 0.9.3 --locked --features cli`; lo llaman los scripts de empaquetado) |
