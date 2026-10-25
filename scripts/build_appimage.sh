#!/usr/bin/env bash
# Empaqueta dist/JASE (salida de `pyinstaller jase.spec`) como AppImage.
# Uso: scripts/build_appimage.sh <dist/JASE> <versión> <appimagetool> <runtime-x86_64>
# El runtime se pasa explícito para que appimagetool no descargue uno "continuous".
set -euo pipefail

src=$1 version=$2 tool=$3 runtime=$4
root=$(cd "$(dirname "$0")/.." && pwd)
appdir=$(mktemp -d)/JASE.AppDir
out="JASE-$version-x86_64.AppImage"

mkdir -p "$appdir/usr/lib"
cp -a "$src" "$appdir/usr/lib/jase"
cp "$root/packaging/linux/jase.desktop" "$appdir/jase.desktop"
cp "$root/src/jase/assets/app_icon.svg" "$appdir/jase.svg"
ln -s jase.svg "$appdir/.DirIcon"
cat > "$appdir/AppRun" <<'APPRUN'
#!/bin/sh
exec "${APPDIR:-$(dirname "$(readlink -f "$0")")}/usr/lib/jase/JASE" "$@"
APPRUN
chmod +x "$appdir/AppRun"

# --appimage-extract-and-run: appimagetool es a su vez un AppImage y el runner no tiene FUSE.
ARCH=x86_64 "$tool" --appimage-extract-and-run --no-appstream --runtime-file "$runtime" \
    "$appdir" "$out"
rm -rf "$(dirname "$appdir")"
echo "$out"
