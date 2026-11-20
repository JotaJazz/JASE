# Publicar una versión

El tag `v<versión>` dispara `.github/workflows/release.yml`: tests, PyInstaller,
instalador Inno Setup, zip portátil y AppImage, cada uno con su prueba de arranque, y una
release en GitHub como **borrador y prerelease** (a propósito).

## Pasos

1. Subir la versión en `pyproject.toml` y en las versiones fijadas de los tests
   (`tests/test_smoke.py`, también como patrón con puntos escapados y en el nombre de
   `test_version_is_X_Y_Z`; `tests/test_workflow.py`) y en el ejemplo de `packaging/jase.iss`;
   buscar con `grep -rn 'X.Y.Z\|X\\.Y\\.Z\|X_Y_Z' tests pyproject.toml packaging`. Luego
   `.venv/bin/pip install -e . --no-deps`.
2. `packaging/release-notes.md`: novedades en inglés y, tras `<!-- ES -->`, en español; los
   tres nombres de archivo de la versión (`JASE-<v>-setup.exe`, `JASE-<v>-win64.zip`,
   `JASE-<v>-x86_64.AppImage`) en las dos mitades (lo exige `tests/test_workflow.py`).
3. Suite completa, ruff, mypy y verificador de la documentación en verde.
4. Commit `build: versión <v>; …`, push de `master` y **esperar al CI verde** antes del tag.
5. `git tag v<v> && git push origin v<v>`: arranca el Release.
6. Revisar el borrador y publicarlo:
   `gh release edit v<v> --draft=false --prerelease=false --latest`.

## Rust (rama `rust`, hasta la 2.0.0)

Los paquetes del binario de Rust (`JASE-<v>-setup.exe`, `JASE-<v>-win64.zip`,
`JASE-<v>-x86_64.AppImage`, con los nombres de 1.3.0) los arma
`.github/workflows/package.yml` en cada push a `rust` que toca `rust/`, `packaging/` o los
scripts de empaquetado, y a mano (`workflow_dispatch`). Solo deja *artifacts*
(`JASE-<v>-win64` y `JASE-<v>-linux-x86_64`): permisos `contents: read`, sin tag ni release
(lo vigila `rust/jase/tests/packaging.rs`). El job `windows` corre además
`scripts/test_upgrade_windows.ps1 -Ci` (instalador y portátil sobre la 1.3.0 publicada).
Bajarlos: `gh run list --workflow package.yml --branch rust` y `gh run download <id>`.

A mano, desde la raíz (necesitan `cargo install cargo-about --version 0.9.3 --locked
--features cli`): `scripts/package_linux.sh <appimagetool> <runtime>` (con
`JASE_GLIBC_MAX=` vacío en un sistema con glibc más nueva que 2.35) y, en Windows con
PowerShell 7 e Inno Setup 6, `./scripts/package_windows.ps1 [-Iscc <ISCC.exe>]`. La versión
sale de `[workspace.package]` en `rust/Cargo.toml` y es la que `update_version()` compara con
las releases. Publicar la 2.0.0 (subir la versión y cambiar `release.yml` a estos
scripts) es la fase 7 del [port](../hoja-de-ruta/port-rust.md).

## Después

- La app avisa de la versión nueva por GitHub Releases (`src/jase/app_update.py`: el
  listado, no `/releases/latest`, porque cuenta las prerelease).
- Lo que no se pudo probar en Windows va a la [deuda](../hoja-de-ruta/deuda.md).
