# Publicar una versión

El tag `v<versión>` dispara `.github/workflows/release.yml`, que llama a
`.github/workflows/package.yml` (`workflow_call`): instalador Inno Setup, zip portátil y
AppImage del binario de Rust, cada uno con su prueba de arranque, la prueba de actualización
desde la 1.3.0 publicada y un paso que falla si el tag no es `v` + la versión de
`cargo pkgid`. Después, el job `release` (solo en tags, el único con `contents: write`) sube
`JASE-<v>-setup.exe`, `JASE-<v>-win64.zip` y `JASE-<v>-x86_64.AppImage` como **borrador y
prerelease**, con `packaging/release-notes.md` de cuerpo. Lo vigila
`rust/jase/tests/packaging.rs`.

El borrador es la puerta: nadie lo ve sin autenticarse. Una prerelease publicada, en cambio,
ya la ofrece el actualizador a todos los usuarios de 1.3.0 y 2.x (`newest_release` lee el
listado de releases y solo descarta los borradores). El SHA-256 que exige el actualizador es
el campo `digest` de cada asset en la API, que pone GitHub al subirlo: las notas no llevan
hashes.

## Pasos (2.0.0 en adelante)

1. Subir `version` en `[workspace.package]` de `rust/Cargo.toml` (los tres crates la
   heredan; de ahí salen *Acerca de*, `--smoke`, el VersionInfo del exe, `/DAppVersion` de
   Inno y los nombres de los paquetes).
2. `packaging/release-notes.md`: novedades en inglés y, tras `<!-- ES -->`, en español; los
   tres nombres de archivo de la versión en las dos mitades (lo exige
   `release_notes_match_version`).
3. Desde `rust/`, con exit 0: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
   `cargo test` (sin pantalla: `xvfb-run -a -s "-screen 0 1920x1080x24" cargo test`), y
   con `--features map-images` para `jase-core`.
4. Commit `build: versión <v>; …` en la rama de trabajo (`rust` u otra), push y CI verde.
   Ensayo sin publicar: `gh workflow run release.yml --ref <rama>` y
   `gh run watch <id> --exit-status`: los dos jobs de *Package* verdes y `release` saltado.
5. En `master`: `git merge --no-ff <rama>`, push, y esperar *CI* y *Package* verdes sobre el
   commit de merge.
6. Comprobar que `cargo pkgid --manifest-path rust/Cargo.toml -p jase` termina en `<v>`;
   `git tag v<v> && git push origin v<v>`: arranca el *Release*.
7. Verificar el borrador:
   `gh api repos/JotaJazz/JASE/releases --jq '.[] | select(.tag_name=="v<v>") | {draft,prerelease,name,assets:[.assets[]|{name,size,digest}]}'`
   → borrador, prerelease, `JASE <v>`, los tres assets con `digest` `sha256:…`. Bajarlos
   (`gh release download v<v> -D <dir>`), comparar `sha256sum` con los `digest` y probarlos
   en Windows (instalador sobre la versión anterior y zip sobre un portátil).
8. Prerelease: `gh release edit v<v> --draft=false --prerelease=true` (desde aquí la ve el
   actualizador). `gh api repos/JotaJazz/JASE/releases/latest --jq .tag_name` sigue en la
   anterior. Probar *Ayuda → Buscar actualizaciones…* desde la versión anterior, instalada y
   portátil.
9. Latest: `gh release edit v<v> --prerelease=false --latest`.

## Si algo sale mal

Nunca se borra ni se mueve el tag, ni se fuerza `master`; los commits malos se deshacen con
`git revert`.

- **No relanzar *Release* sobre un tag ya publicado** (su primer paso falla a propósito): un
  arreglo sale como la versión siguiente.
- **Borrador malo:** `gh release delete v<v> --yes` (sin `--cleanup-tag`) y el arreglo sale
  como la versión siguiente (p. ej. 2.0.1), desde el paso 1.
- **Ya pública:** `gh release edit v<v> --draft=true` (desaparece del listado anónimo y el
  actualizador deja de ofrecerla); si era *Latest*, `gh release edit v<anterior> --latest`.
  Quien ya la instaló no vuelve solo (nunca se ofrece una versión menor): el arreglo es la
  versión siguiente publicada, o reinstalar a mano el `setup.exe` anterior (mismo AppId; la
  config se lee igual).

## A mano

Los paquetes también se arman sin tag: `package.yml` corre en cada push a `master` o `rust`
que toca `rust/`, `packaging/` o los scripts de empaquetado, y con `workflow_dispatch`; solo
deja *artifacts* (`JASE-<v>-win64` y `JASE-<v>-linux-x86_64`). Bajarlos:
`gh run list --workflow package.yml --branch <rama>` y `gh run download <id>`.

En local, desde la raíz (necesitan `cargo install cargo-about --version 0.9.3 --locked
--features cli`): `scripts/package_linux.sh <appimagetool> <runtime>` (con
`JASE_GLIBC_MAX=` vacío en un sistema con glibc más nueva que 2.35) y, en Windows con
PowerShell 7 e Inno Setup 6, `./scripts/package_windows.ps1 [-Iscc <ISCC.exe>]`.

La receta de 1.3.0 y anteriores (Python y PyInstaller) quedó en la tag `v1.3.0`.

## Después

- La app avisa de la versión nueva por GitHub Releases (`rust/jase-core/src/app_update.rs`: el
  listado, no `/releases/latest`, porque cuenta las prerelease).
- Lo que no se pudo probar en Windows va a la [deuda](../hoja-de-ruta/deuda.md).
