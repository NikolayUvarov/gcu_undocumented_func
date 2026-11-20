#!/usr/bin/env python3
"""Fetch/check the pinned RTL8852B Bluetooth patch (bt_hci, 700-BLT-0004) into the ignored store."""

import argparse
import sys
from pathlib import Path

sys.dont_write_bytecode = True  # no __pycache__ beside the scripts
from rtl8852be import BASE, ensure_asset  # noqa: E402

STORE = Path(__file__).resolve().parents[2] / "proprietary/firmware/rtl_bt"
# The same pinned linux-firmware revision as the RTL8852BE's; WHENCE's rtl_bt section: "Redistributable. See
# LICENCE.rtlwifi_firmware.txt for details."
ASSETS = (
    ("LICENCE.rtlwifi_firmware.txt", "LICENCE.rtlwifi_firmware.txt", 2115,
     "a61351665b4f264f6c631364f85b907d8f8f41f8b369533ef4021765f9f3b62e"),
    # No configuration file: linux-firmware has none for this controller, and its patch needs none.
    ("rtl8852bu_fw.bin", "rtl_bt/rtl8852bu_fw.bin", 127301,
     "fa1a98b9623ee1ef907541429a486c52ca124062a6465500557495a6b3f14cba"),
)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("fetch", "check"))
    args = parser.parse_args()
    try:
        for asset in ASSETS:
            path = ensure_asset(STORE, asset, args.action == "fetch")
            print(f"verified {asset[3]}  {path}")
    except (OSError, ValueError) as error:
        parser.exit(1, f"rtl8852b bluetooth firmware: {error}\n")
    print(f"Personal store only ({BASE}); no disk or image was changed.")


if __name__ == "__main__":
    main()
