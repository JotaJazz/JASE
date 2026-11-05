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

## Después

- La app avisa de la versión nueva por GitHub Releases (`src/jase/app_update.py`: el
  listado, no `/releases/latest`, porque cuenta las prerelease).
- Lo que no se pudo probar en Windows va a la [deuda](../hoja-de-ruta/deuda.md).
