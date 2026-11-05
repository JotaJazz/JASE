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

## Ejecutar

| Qué | Comando |
|---|---|
| App (WSLg en Linux) | `.venv/bin/python -m jase` — desde Linux no se lanzan juegos de Windows salvo con Wine |
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
| Nombres de países | `.venv/bin/python scripts/gen_countries_en.py` (Qt) · `python scripts/gen_countries.py pt fr de it` (necesita `babel`) |
| Avisos de terceros | `python scripts/collect_licenses.py <salida>` (lo usa el Release) |
| AppImage | `scripts/build_appimage.sh <dist/JASE> <versión> <appimagetool> <runtime>` (lo usa el Release) |
