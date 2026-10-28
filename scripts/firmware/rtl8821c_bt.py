#!/usr/bin/env python3
"""Fetch/check the pinned RTL8821C Bluetooth patch and configuration (bt_hci, 700-BLT-0001) into the ignored store."""

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
    ("rtl8821c_fw.bin", "rtl_bt/rtl8821c_fw.bin", 54636,
     "dfe7ef772d078ec09490c02190a32fcd40b3eaf3b040049d8930eb80e6d5743c"),
    ("rtl8821c_config.bin", "rtl_bt/rtl8821c_config.bin", 10,
     "292717b0610ce920f66b734c299d349ea59a4c4400044078d8fc65625f17e382"),
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
        parser.exit(1, f"rtl8821c bluetooth firmware: {error}\n")
    print(f"Personal store only ({BASE}); no disk or image was changed.")


if __name__ == "__main__":
    main()
