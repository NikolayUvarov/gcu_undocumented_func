#!/usr/bin/env bash
# The aarch64 image for a USB drive (dist/mind-core-usb-aarch64.img): builds with ARCH=aarch64, then packages
# aarch64_root/ as 04_make_usb_image.sh does usb_root/. Arguments go to scripts/make_usb_image.py (--no-build, --force,
# --output). Write it with ./05_write_usb_linux.sh --image dist/mind-core-usb-aarch64.img (Windows: -Image).
set -euo pipefail

USB_SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
exec python3 "$USB_SCRIPT_DIR/scripts/make_usb_image.py" --arch aarch64 "$@"
