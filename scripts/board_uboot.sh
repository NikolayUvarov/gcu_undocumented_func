#!/usr/bin/env bash
# U-Boot with TF-A's BL31 for an Allwinner board's card (100004-MOB-0001): pinned tags, built once into a cache, no root.
# Usage: scripts/board_uboot.sh [--fetch-tools] orangepi-zero-plus2|orangepi-zero3   (prints u-boot-sunxi-with-spl.bin's path)
# U-Boot's network is off (CONFIG_NO_NET): no DHCP, PXE or TFTP boot runs before our loader (351-KRN-0195).
# Needs aarch64-linux-gnu-gcc, make, bison, flex, swig, bc, python3 with setuptools and pyelftools, OpenSSL's headers.
# --fetch-tools: bison, swig and pyelftools as Ubuntu packages unpacked into the cache (apt-get download, no root).
# MIND_BOARD_UBOOT=<file> uses that file instead; the cache is ${XDG_CACHE_HOME:-$HOME/.cache}/mind-board-uboot.
set -euo pipefail
TFA_TAG=v2.15.0 TFA_COMMIT=da738d5eae93af342fdc4995dd3c05acb4c9d757
TFA_URLS=("https://git.trustedfirmware.org/TF-A/trusted-firmware-a.git" "https://github.com/ARM-software/arm-trusted-firmware.git")
UBOOT_TAG=v2026.10 UBOOT_COMMIT=5508406582f6f7120dce0c4b3059819a41788db2
UBOOT_URLS=("https://source.denx.de/u-boot/u-boot.git" "https://github.com/u-boot/u-boot.git")

FETCH_TOOLS=0
[[ ${1:-} == --fetch-tools ]] && { FETCH_TOOLS=1; shift; }
BOARD=${1:-}
# The board: TF-A's platform, U-Boot's defconfig, and the bytes this script made on Ubuntu 24.04's cross gcc 13.3.0.
case $BOARD in
    orangepi-zero-plus2) PLAT=sun50i_a64 DEFCONFIG=orangepi_zero_plus2_defconfig
                         KNOWN=f229497abae79a12cf3e406defe240e01bc20a71fa1a5dd80048dc66496b9e1f ;;
    orangepi-zero3) PLAT=sun50i_h616 DEFCONFIG=orangepi_zero3_defconfig
                    KNOWN=6902f66c911e637f40e8f87ef59cb248d98182a81737ccd94efdefe7d03e7d24 ;;
    *) echo "usage: $0 [--fetch-tools] orangepi-zero-plus2|orangepi-zero3" >&2; exit 2 ;;
esac
if [[ -n ${MIND_BOARD_UBOOT:-} ]]; then [[ -f $MIND_BOARD_UBOOT ]] || { echo "MIND_BOARD_UBOOT=$MIND_BOARD_UBOOT: no such file" >&2; exit 1; }; echo "$MIND_BOARD_UBOOT"; exit 0; fi
CACHE=${XDG_CACHE_HOME:-$HOME/.cache}/mind-board-uboot
OUT=$CACHE/$UBOOT_TAG-$TFA_TAG/$BOARD
BIN=$OUT/u-boot-sunxi-with-spl.bin
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
WORK=$CACHE/build/$UBOOT_TAG-$TFA_TAG/$BOARD
rm -rf "$WORK"
mkdir -p "$WORK" "$OUT"
SOURCE_DATE_EPOCH=$(git -C "$SRC/tf-a-$TFA_TAG" log -1 --format=%ct) \
    make -s -C "$SRC/tf-a-$TFA_TAG" BUILD_BASE="$WORK/tf-a" PLAT="$PLAT" DEBUG=0 -j"$(nproc)" bl31 >&2
BL31=$WORK/tf-a/$PLAT/release/bl31.bin
UB=(make -s -C "$SRC/u-boot-$UBOOT_TAG" O="$WORK/u-boot" BL31="$BL31" SCP=/dev/null)
export SOURCE_DATE_EPOCH=$(git -C "$SRC/u-boot-$UBOOT_TAG" log -1 --format=%ct)
"${UB[@]}" "$DEFCONFIG" >&2
# No network in the firmware; no capsule tool, which would need GnuTLS's headers.
"$SRC/u-boot-$UBOOT_TAG/scripts/config" --file "$WORK/u-boot/.config" --enable NO_NET --disable TOOLS_MKEFICAPSULE
"${UB[@]}" olddefconfig >&2
"${UB[@]}" -j"$(nproc)" all u-boot-initial-env >&2
grep -q '^CONFIG_EFI_LOADER=y' "$WORK/u-boot/.config" || { echo "board_uboot: $DEFCONFIG has no UEFI loader" >&2; exit 1; }
if grep -a -q -E '^boot_targets=.*\b(pxe|dhcp)\b' "$WORK/u-boot/u-boot-initial-env"; then echo "board_uboot: U-Boot still boots from the network" >&2; exit 1; fi
cp "$WORK/u-boot/.config" "$OUT/u-boot.config"
cp "$WORK/u-boot/u-boot-initial-env" "$OUT/"
cp "$BL31" "$OUT/bl31.bin"
cp "$WORK/u-boot/u-boot-sunxi-with-spl.bin" "$BIN.new"
got=$(sha256sum "$BIN.new" | cut -d' ' -f1)
if [[ $got == "$KNOWN" ]]; then echo "board_uboot: $BOARD: the bytes known from Ubuntu 24.04's cross gcc 13.3.0" >&2
else echo "board_uboot: $BOARD: SHA-256 $got, not the $KNOWN made on Ubuntu 24.04's cross gcc 13.3.0 (another toolchain?)" >&2; fi
mv "$BIN.new" "$BIN"
echo "$BIN"
