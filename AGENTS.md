# AGENTS.md — JASE

Navegador de servidores de juegos clásicos (Rust + Iced), estilo All-Seeing Eye.
Uso y juegos: `README.md`. Documentación técnica: `docs/README.md`.

## Leer antes de cambiar código
1. `docs/ai/README_AI.md` (orden de lectura de la capa de IA).
2. `docs/arquitectura/limites.md` (reglas de dependencias, hilos y datos remotos).
3. `docs/hoja-de-ruta/deuda.md` (lo que ya se sabe que falta o está sin verificar).

## Reglas duras
- Desde `rust/`, con exit 0 y salida limpia: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` (sin pantalla: `xvfb-run -a -s "-screen 0 1920x1080x24" cargo test`); para imágenes de mapas, también `clippy` y `test` de `jase-core` con `--features map-images`.
- Núcleo sin UI: `jase-core` no depende de Iced, keyring ni imágenes (salvo la feature `map-images`); lo vigila `cargo tree` en `rust/jase/tests/deps.rs`.
- Nada de red, DNS, lectura de carpetas de juegos, extracción de imágenes ni archivos grandes en `update`/`view`: van en `Task`/`spawn_blocking` (la config, pequeña, sí se guarda ahí); resultados tardíos descartados por generación o número de pedido.
- Texto de servidores remotos como texto plano; argumentos al juego validados.
- Cada texto visible con `tr`/`tr_fmt`/`tr_n` (clave en español) y traducido en los 5 catálogos de `rust/jase-core/locales/`.
- Columnas por `enum Column`; colores como campos de `Palette` en todos los temas de `rust/jase/src/theme/palette.rs`.
- Un juego o master entra solo con un sondeo real y capturas en `tests/fixtures/`.
- Masters iguales en `masters.json` y `rust/jase-core/src/games_table.rs`.
- Nunca versionar archivos de los juegos ni `docs/superpowers/` (notas locales).

## Sincronía con la documentación
Un cambio que deja una doc incorrecta la corrige en el mismo commit; cada trabajo
terminado agrega una etapa a `docs/ai/PROJECT_MEMORY.md`. Tabla «si cambias X, actualiza
Y»: `docs/ai/DEVELOPMENT_GUIDE.md`. El verificador `rust/jase/tests/check_docs.rs` corre con `cargo test`.

## Comandos
`docs/desarrollo/comandos.md`. Publicar: `docs/desarrollo/publicar.md`. Agregar un juego:
`docs/desarrollo/agregar-juego.md`.
