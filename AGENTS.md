# AGENTS.md — JASE

Navegador de servidores de juegos clásicos (Python + PySide6), estilo All-Seeing Eye.
Uso y juegos: `README.md`. Documentación técnica: `docs/README.md`.

## Leer antes de cambiar código
1. `docs/ai/README_AI.md` (orden de lectura de la capa de IA).
2. `docs/arquitectura/limites.md` (reglas de dependencias, hilos y datos remotos).
3. `docs/hoja-de-ruta/deuda.md` (lo que ya se sabe que falta o está sin verificar).

## Reglas duras
- Tests verdes y limpios: `.venv/bin/pytest -q -W error`; también `ruff check src tests scripts` y `mypy`.
- Núcleo sin Qt: `cli.py` y lo que importa no usan PySide6, keyring ni Pillow.
- Nada de red ni disco en el hilo de la UI; resultados tardíos descartados por generación o identidad.
- Texto de servidores remotos como texto plano; argumentos al juego validados.
- Cada texto visible con `tr()`/`tr_n()` (clave en español) y traducido en los 5 catálogos de `src/jase/locales/`.
- Columnas por `COL_*`; colores como tokens en todos los temas de `src/jase/ui/theme.py`.
- Un juego o master entra solo con un sondeo real y capturas en `tests/fixtures/`.
- Masters iguales en `masters.json`, `src/jase/assets/masters.json` y `src/jase/games.py`.
- Nunca versionar archivos de los juegos ni `docs/superpowers/` (notas locales).
- Sin `@Slot` en los mixins `src/jase/ui/window_*.py`.

## Sincronía con la documentación
Un cambio que deja una doc incorrecta la corrige en el mismo commit; cada trabajo
terminado agrega una etapa a `docs/ai/PROJECT_MEMORY.md`. Tabla «si cambias X, actualiza
Y»: `docs/ai/DEVELOPMENT_GUIDE.md`. `scripts/check_docs.py` corre con los tests.

## Comandos
`docs/desarrollo/comandos.md`. Publicar: `docs/desarrollo/publicar.md`. Agregar un juego:
`docs/desarrollo/agregar-juego.md`.
