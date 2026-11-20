# Estado del sistema

Actualizado: 2026-11-20. Versión: **2.0.0** (Rust + Iced), publicada el 2026-11-20
desde `master`; la anterior es la **1.3.0** (2026-11-05, Python + PySide6, en la tag `v1.3.0`). La 2.0.0
lee la misma config, favoritos, listas y contraseñas, y la instala el actualizador de 1.3.0.

## Árbol

```
rust/jase-core/   núcleo sin UI: protocols/ (6 familias + huffman + steamweb), scanner/,
                  maps/, games_table.rs, config, masters, i18n, …; locales/ (en, pt, fr, de,
                  it + nombres de países); tests/ (+ golden/: resultados fijos)
rust/jase-cli/    binario jase-cli (no va en los paquetes)
rust/jase/        app Iced: app/ (estado y update por dominio), table/, theme/, tray/,
                  diálogos; assets/ (banderas, iconos, temas); tests/ (incluye check_docs.rs,
                  parity_docs.rs, packaging.rs)
masters.json      lista oficial de masters (también incluida en el binario)
tests/fixtures/   capturas reales (*.bin) que usan los tests de Rust
scripts/          empaquetado: package_linux.sh, package_windows.ps1, build_appimage.sh,
                  test_upgrade_windows.ps1
packaging/        instalador Inno Setup, licencias, notas de versión, .desktop
.github/workflows/  ci.yml, package.yml, release.yml
docs/             esta documentación (+ docs/superpowers/: notas locales, no versionadas)
```

## Juegos (36) por familia

| Familia | N.º | Juegos |
|---|---|---|
| `quake3` | 17 | cod2, cod1, coduo, cod4, q3, rtcw, et, urt, oa, jka, sof2, jk2, xonotic, warsow, warfork, tremulous, smokinguns |
| `valve` | 9 | cs16, l4d, l4d2, css, tf2, hldm, dod, tfc, gmod (los otros ocho por la API web de Steam, con clave; cs16 la usa como opcional) |
| `gamespy` | 6 | ut99, tacops, mohaa, mohaas, mohaab, bf1942 |
| `quake2` | 2 | q2, dday |
| `ut2004` | 1 | ut2004 |
| `zandronum` | 1 | zandronum |

## Estado por subsistema

| Subsistema | Estado |
|---|---|
| Escáner y protocolos | estable; 6 familias |
| UI | Iced 0.14, paridad con 1.3.0 salvo las divergencias aceptadas de [port-rust.md](../hoja-de-ruta/port-rust.md) (sin lector de pantalla) |
| CLI (`jase-cli`) | estable; contrato JSON schema 1 ([cli.md](../cli.md)) |
| Actualización desde la app | instalador y zip portátil; desde 1.3.0, probada en el CI; de 2.0.0 a una 2.0.x, sin probar hasta que exista una ([deuda](../hoja-de-ruta/deuda.md)) |
| Empaquetado | Windows (instalador + zip) y Linux (AppImage), con `package.yml` |
| i18n | 6 idiomas |
| Documentación | verificador en `cargo test` (`rust/jase/tests/check_docs.rs`) |

## Calidad

CI (`ci.yml`): job Rust en Ubuntu y Windows: `cargo fmt --check`, `clippy -D warnings` y
`cargo test` (en Linux con `xvfb-run`), más `clippy` y `test` de `jase-core` con
`--features map-images`. *Package* (`package.yml`) arma y prueba los tres paquetes.

## Contratos

| Contrato | Dónde |
|---|---|
| Salida JSON de `jase-cli` (schema 1) | [docs/cli.md](../cli.md) |
| Enlaces `jase://connect/<juego>/<host>:<puerto>` | `rust/jase-core/src/jase_url.rs` |
| `masters.json` (schema 1, `updated`) | `rust/jase-core/src/masters.rs` |
| Listas guardadas (schema 1) | `rust/jase-core/src/server_cache.rs` |
| Exportación de favoritos (`jase-favorites`, versión 1) | `rust/jase-core/src/favorites_io.rs` |
