#!/usr/bin/env bash
# U-Boot's qemu_arm64 build for the device-tree group (100000-PRT-0009): one pinned build on every machine, without root.
# Fetches Ubuntu 24.04's u-boot-qemu package once into a cache, checks it by SHA-256, keeps u-boot.bin and prints its path.
# MIND_UBOOT=<file> uses that file instead; the cache is ${XDG_CACHE_HOME:-$HOME/.cache}/mind-u-boot.
set -euo pipefail
VERSION=2025.10-0ubuntu0.24.04.4
DEB=u-boot-qemu_${VERSION}_all.deb
DEB_SHA256=997cd55ca82e64163595249d1b8117732e6ad4c22a67634e605286cc61508c27
BIN_SHA256=c06841c0f6566f27dac0e71182011216883a30158a2de31614937f73b10ec04c
SOURCES=("http://archive.ubuntu.com/ubuntu/pool/main/u/u-boot/$DEB" "https://launchpad.net/ubuntu/+archive/primary/+files/$DEB")

if [[ -n ${MIND_UBOOT:-} ]]; then [[ -f $MIND_UBOOT ]] || { echo "MIND_UBOOT=$MIND_UBOOT: no such file" >&2; exit 1; }; echo "$MIND_UBOOT"; exit 0; fi
DIR=${XDG_CACHE_HOME:-$HOME/.cache}/mind-u-boot/$VERSION
BIN=$DIR/qemu_arm64/u-boot.bin
if [[ -f $BIN ]] && echo "$BIN_SHA256  $BIN" | sha256sum -c --quiet - 2>/dev/null; then echo "$BIN"; exit 0; fi
mkdir -p "$DIR"
WORK=$(mktemp -d "$DIR/fetch.XXXXXX")
trap 'rm -rf "$WORK"' EXIT
fetched=
for url in "${SOURCES[@]}"; do
    if curl -fsSL --retry 2 -o "$WORK/$DEB" "$url" && echo "$DEB_SHA256  $WORK/$DEB" | sha256sum -c --quiet -; then fetched=$url; break; fi
    echo "fetch_uboot: $url: not fetched, or not the pinned bytes" >&2
done
[[ -n $fetched ]] || { echo "fetch_uboot: $DEB from none of its sources" >&2; exit 1; }
# A .deb is an ar archive; its data member holds usr/lib/u-boot/qemu_arm64/u-boot.bin.
if command -v dpkg-deb >/dev/null; then dpkg-deb -x "$WORK/$DEB" "$WORK/x"; else
    (cd "$WORK" && ar x "$DEB" && mkdir x && tar -C x -xf data.tar.*); fi
echo "$BIN_SHA256  $WORK/x/usr/lib/u-boot/qemu_arm64/u-boot.bin" | sha256sum -c --quiet -
mkdir -p "$DIR/qemu_arm64"
cp "$WORK/x/usr/lib/u-boot/qemu_arm64/u-boot.bin" "$BIN.new" && mv "$BIN.new" "$BIN"
cp "$WORK/x/usr/share/doc/u-boot-qemu/copyright" "$DIR/copyright"
echo "fetch_uboot: $DEB from $fetched" >&2
echo "$BIN"
