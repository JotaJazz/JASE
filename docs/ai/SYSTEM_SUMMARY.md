# Estado del sistema

Actualizado: 2026-11-05. Versión publicada: **1.2.1** (2026-10-31). `master` lleva trabajo
sin publicar: auditorías, masters nuevos, mejoras de la UI y listas guardadas.

## Árbol

```
src/jase/        núcleo (sin Qt) + __main__.py (app) + cli.py (jase-cli)
src/jase/protocols/  parsers por familia (6) + huffman.py + steamweb.py
src/jase/maps/   imágenes de mapas
src/jase/ui/     MainWindow + 21 mixins window_*.py, workers, cargadores, tema
src/jase/locales/  traducciones en, pt, fr, de, it (+ nombres de países)
src/jase/assets/  masters.json empaquetado, banderas, iconos, temas
tests/           pytest (+ fixtures/*.bin con capturas reales)
scripts/         sondeos, iconos, capturas, licencias, AppImage, check_docs.py
packaging/       instalador Inno Setup, licencias, notas de versión, .desktop
.github/workflows/  ci.yml, release.yml
docs/            esta documentación (+ docs/superpowers/: notas locales, no versionadas)
```

## Juegos (36) por familia

| Familia | N.º | Juegos |
|---|---|---|
| `quake3` | 17 | cod2, cod1, coduo, cod4, q3, rtcw, et, urt, oa, jka, sof2, jk2, xonotic, warsow, warfork, tremulous, smokinguns |
| `valve` | 9 | cs16, l4d, l4d2, css, tf2, hldm, dod, tfc, gmod (los ocho de Source por la API web de Steam, con clave) |
| `gamespy` | 6 | ut99, tacops, mohaa, mohaas, mohaab, bf1942 |
| `quake2` | 2 | q2, dday |
| `ut2004` | 1 | ut2004 |
| `zandronum` | 1 | zandronum |

## Estado por subsistema

| Subsistema | Estado |
|---|---|
| Escáner y protocolos | estable; 6 familias |
| UI | estable; listas guardadas entre sesiones nuevas (sin publicar) |
| CLI (`jase-cli`) | estable; contrato JSON schema 1 ([cli.md](../cli.md)) |
| Actualización desde la app | instalador y zip portátil; portátil sin prueba completa en Windows |
| Empaquetado | Windows (instalador + zip) y Linux (AppImage) en el Release |
| i18n | 6 idiomas |
| Documentación | sistema nuevo (2026-11-05) con verificador en los tests |

## Calidad

CI (`ci.yml`): Ubuntu + Python 3.11 + PySide6 6.11.2 y Windows + Python 3.13 + PySide6
6.12.0; ruff, mypy y pytest. Suite: ~1720 tests.

## Contratos

| Contrato | Dónde |
|---|---|
| Salida JSON de `jase-cli` (schema 1) | [docs/cli.md](../cli.md) |
| Enlaces `jase://connect/<juego>/<host>:<puerto>` | `src/jase/jase_url.py` |
| `masters.json` (schema 1, `updated`) | `src/jase/masters.py` |
| Listas guardadas (schema 1) | `src/jase/server_cache.py` |
| Exportación de favoritos (`jase-favorites`, versión 1) | `src/jase/favorites_io.py` |
