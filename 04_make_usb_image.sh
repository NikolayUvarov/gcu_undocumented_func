#!/usr/bin/env bash
set -euo pipefail

USB_SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
exec python3 "$USB_SCRIPT_DIR/scripts/make_usb_image.py" "$@"
