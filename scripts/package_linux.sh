#!/usr/bin/env bash
# Empaqueta el binario de Rust como AppImage: deja dist/JASE y JASE-<versión>-x86_64.AppImage.
# Uso: scripts/package_linux.sh <appimagetool> <runtime-x86_64>
# JASE_GLIBC_MAX (por defecto 2.35, la glibc de ubuntu-22.04): el GLIBC_x.y más nuevo que puede
# pedir el binario; vacío, sin tope (para probar en un sistema más nuevo).
# Con `source` solo define check_elf.
set -euo pipefail

# Falla si el binario enlaza algo fuera de glibc o pide un símbolo de una glibc más nueva:
# el AppImage no lleva librerías, así que el binario solo puede contar con estas.
check_elf() {
    local bin=$1 cap=GLIBC_${JASE_GLIBC_MAX-2.35} dyn ver lib max
    dyn=$(readelf -d "$bin") || return 1
    ver=$(readelf -V "$bin") || return 1
    for lib in $(sed -n 's/.*(NEEDED).*\[\(.*\)\]/\1/p' <<< "$dyn"); do
        case $lib in
            libc.so.6 | libm.so.6 | libgcc_s.so.1 | ld-linux-x86-64.so.2) ;;
            *) echo "$bin enlaza $lib, que el AppImage no lleva" >&2; return 1 ;;
        esac
    done
    [ "$cap" = GLIBC_ ] && return 0
    max=$(grep -o 'GLIBC_[0-9][0-9.]*' <<< "$ver" | sort -uV | tail -n 1) || true
    if [ -n "$max" ] && [ "$max" != "$cap" ] \
        && [ "$(printf '%s\n%s\n' "$max" "$cap" | sort -V | tail -n 1)" = "$max" ]; then
        echo "$bin pide $max, más nuevo que $cap" >&2
        return 1
    fi
}

[[ ${BASH_SOURCE[0]} != "$0" ]] && return 0

# Los argumentos, relativos a donde se llama; el resto, a la raíz del repo.
tool=$(realpath "$1") runtime=$(realpath "$2")
cd "$(dirname "$0")/.."

pkgid=$(cd rust && cargo pkgid -p jase)
version=${pkgid##*[#@]}

# Desde rust/: así aplica rust/.cargo/config.toml.
(cd rust && cargo build --release --locked -p jase)
bin=${CARGO_TARGET_DIR:-rust/target}/release/jase
check_elf "$bin"

rm -rf dist/JASE
mkdir -p dist/JASE/licenses
cp "$bin" dist/JASE/JASE
chmod 755 dist/JASE/JASE
for f in JASE-LICENSE.txt DB-IP.txt flag-icons-MIT.txt; do
    cp "packaging/licenses/$f" dist/JASE/licenses/
done
(cd rust && cargo about generate -c about.toml about.hbs -o ../dist/JASE/licenses/THIRD-PARTY.txt)

scripts/build_appimage.sh dist/JASE "$version" "$tool" "$runtime"
out="JASE-$version-x86_64.AppImage"

smoke=$(mktemp)
trap 'rm -f "$smoke"' EXIT
if ! ICED_BACKEND=tiny-skia "./$out" --appimage-extract-and-run --smoke > "$smoke"; then
    cat "$smoke"
    echo "El AppImage falló el smoke test" >&2
    exit 1
fi
grep -qx "JASE smoke OK $version" "$smoke" || {
    cat "$smoke"
    echo "El AppImage no escribió «JASE smoke OK $version»" >&2
    exit 1
}
echo "$out"
