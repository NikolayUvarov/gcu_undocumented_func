#!/usr/bin/env bash
# U-Boot with TF-A's BL31 for an Allwinner board's card (100004-MOB-0001): pinned tags, built once into a cache, no root.
# Usage: scripts/board_uboot.sh [--fetch-tools] <board>   (prints the firmware's path; the boards: board_image.py --list)
# The board's defconfig, TF-A platform and file, the sources' tags, commits and URLs and the bytes known come from its entry in
# scripts/boards.toml, the catalog of devices (352-MOB-0010); this script holds the build's settings.
# U-Boot's network is off (CONFIG_NO_NET): no DHCP, PXE or TFTP boot runs before our loader (351-KRN-0195).
# Its countdown stops only on the typed word `stop` (CONFIG_AUTOBOOT_KEYED): the console is also the link's line, whose
# frames would stop it as any key does.
# Needs aarch64-linux-gnu-gcc, make, bison, flex, swig, bc, python3 (3.11 or later) with setuptools and pyelftools, OpenSSL's headers.
# --fetch-tools: bison, swig and pyelftools as Ubuntu packages unpacked into the cache (apt-get download, no root).
# MIND_BOARD_UBOOT=<file> uses that file instead; the cache is ${XDG_CACHE_HOME:-$HOME/.cache}/mind-board-uboot.
set -euo pipefail
ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
SETTINGS=2  # the settings below; a new number builds again instead of taking the cached files

FETCH_TOOLS=0
[[ ${1:-} == --fetch-tools ]] && { FETCH_TOOLS=1; shift; }
BOARD=${1:-}
usage() { echo "usage: $0 [--fetch-tools] <board>; the boards: $(python3 "$ROOT/scripts/board_image.py" --list | cut -f1 | tr '\n' ' ')" >&2; exit 2; }
[[ $BOARD =~ ^[a-z0-9-]+$ ]] || usage
# The board's entry: TF-A's platform, U-Boot's defconfig, the sources, and the bytes known from the toolchain it names.
assignments=$(python3 "$ROOT/scripts/board_image.py" --board "$BOARD" --shell) || usage
eval "$assignments"
if [[ -n ${MIND_BOARD_UBOOT:-} ]]; then [[ -f $MIND_BOARD_UBOOT ]] || { echo "MIND_BOARD_UBOOT=$MIND_BOARD_UBOOT: no such file" >&2; exit 1; }; echo "$MIND_BOARD_UBOOT"; exit 0; fi
CACHE=${XDG_CACHE_HOME:-$HOME/.cache}/mind-board-uboot
OUT=$CACHE/$UBOOT_TAG-$TFA_TAG-s$SETTINGS/$BOARD
BIN=$OUT/$FILE
[[ -f $BIN ]] && { echo "$BIN"; exit 0; }

# The tools, fetched as packages into the cache when asked; the build then finds them first.
TOOLS=$CACHE/tools
if (( FETCH_TOOLS )); then
    mkdir -p "$TOOLS/debs"
    (cd "$TOOLS/debs" && apt-get download bison swig python3-pyelftools >&2)
    for deb in "$TOOLS"/debs/*.deb; do dpkg-deb -x "$deb" "$TOOLS/root"; done
fi
if [[ -d $TOOLS/root ]]; then
    export PATH="$TOOLS/root/usr/bin:$PATH" BISON_PKGDATADIR="$TOOLS/root/usr/share/bison"
    export PYTHONPATH="$TOOLS/root/usr/lib/python3/dist-packages${PYTHONPATH:+:$PYTHONPATH}"
    for lib in "$TOOLS"/root/usr/share/swig*; do export SWIG_LIB=$lib; done
fi
missing=()
for tool in aarch64-linux-gnu-gcc make bison flex swig bc python3 git; do command -v "$tool" >/dev/null || missing+=("$tool"); done
python3 -c "import elftools, setuptools" 2>/dev/null || missing+=("python3's elftools or setuptools")
echo '#include <openssl/evp.h>' | cc -E - >/dev/null 2>&1 || missing+=("OpenSSL's headers")
if (( ${#missing[@]} )); then
    echo "board_uboot: missing ${missing[*]}" >&2
    echo "  Ubuntu: apt install gcc-aarch64-linux-gnu make bison flex swig bc python3-dev python3-setuptools python3-pyelftools libssl-dev," >&2
    echo "  or run again with --fetch-tools for bison, swig and pyelftools without root" >&2
    exit 1
fi

# The sources at their tags, each checked against its commit.
fetch() {
    local dir=$1 tag=$2 commit=$3; shift 3
    if [[ ! -d $dir/.git ]]; then
        local url
        for url in "$@"; do git clone -q --depth 1 --branch "$tag" "$url" "$dir.new" 2>/dev/null && break; rm -rf "$dir.new"; done
        [[ -d $dir.new ]] || { echo "board_uboot: $tag from none of $*" >&2; exit 1; }
        mv "$dir.new" "$dir"
    fi
    [[ $(git -C "$dir" rev-parse HEAD) == "$commit" ]] || { echo "board_uboot: $dir is not $tag's commit $commit" >&2; exit 1; }
}
SRC=$CACHE/src
mkdir -p "$SRC"
fetch "$SRC/tf-a-$TFA_TAG" "$TFA_TAG" "$TFA_COMMIT" "${TFA_URLS[@]}"
fetch "$SRC/u-boot-$UBOOT_TAG" "$UBOOT_TAG" "$UBOOT_COMMIT" "${UBOOT_URLS[@]}"

# The build: each tree's commit time as its time, a fixed locale and zone, outside the sources.
export LC_ALL=C TZ=UTC KBUILD_BUILD_USER=mind KBUILD_BUILD_HOST=mind-build CROSS_COMPILE=aarch64-linux-gnu-
WORK=$CACHE/build/$UBOOT_TAG-$TFA_TAG-s$SETTINGS/$BOARD
rm -rf "$WORK"
mkdir -p "$WORK" "$OUT"
SOURCE_DATE_EPOCH=$(git -C "$SRC/tf-a-$TFA_TAG" log -1 --format=%ct) \
    make -s -C "$SRC/tf-a-$TFA_TAG" BUILD_BASE="$WORK/tf-a" PLAT="$PLAT" DEBUG=0 -j"$(nproc)" bl31 >&2
BL31=$WORK/tf-a/$PLAT/release/bl31.bin
UB=(make -s -C "$SRC/u-boot-$UBOOT_TAG" O="$WORK/u-boot" BL31="$BL31" SCP=/dev/null)
export SOURCE_DATE_EPOCH=$(git -C "$SRC/u-boot-$UBOOT_TAG" log -1 --format=%ct)
"${UB[@]}" "$DEFCONFIG" >&2
# No network in the firmware; the countdown stopped by `stop` only; no capsule tool, which would need GnuTLS's headers.
"$SRC/u-boot-$UBOOT_TAG/scripts/config" --file "$WORK/u-boot/.config" --enable NO_NET --disable TOOLS_MKEFICAPSULE \
    --enable AUTOBOOT_KEYED --set-str AUTOBOOT_STOP_STR stop --set-str AUTOBOOT_PROMPT 'Autoboot in %d s: type stop for the U-Boot prompt\n'
"${UB[@]}" olddefconfig >&2
"${UB[@]}" -j"$(nproc)" all u-boot-initial-env >&2
grep -q '^CONFIG_EFI_LOADER=y' "$WORK/u-boot/.config" || { echo "board_uboot: $DEFCONFIG has no UEFI loader" >&2; exit 1; }
grep -q '^CONFIG_AUTOBOOT_STOP_STR="stop"' "$WORK/u-boot/.config" || { echo "board_uboot: the countdown would stop on any key" >&2; exit 1; }
if grep -a -q -E '^boot_targets=.*\b(pxe|dhcp)\b' "$WORK/u-boot/u-boot-initial-env"; then echo "board_uboot: U-Boot still boots from the network" >&2; exit 1; fi
cp "$WORK/u-boot/.config" "$OUT/u-boot.config"
cp "$WORK/u-boot/u-boot-initial-env" "$OUT/"
cp "$BL31" "$OUT/bl31.bin"
cp "$WORK/u-boot/$FILE" "$BIN.new"
got=$(sha256sum "$BIN.new" | cut -d' ' -f1)
if [[ $got == "$KNOWN" ]]; then echo "board_uboot: $BOARD: the bytes known from $KNOWN_WITH" >&2
else echo "board_uboot: $BOARD: SHA-256 $got, not the $KNOWN made with $KNOWN_WITH (another toolchain?)" >&2; fi
mv "$BIN.new" "$BIN"
echo "$BIN"
