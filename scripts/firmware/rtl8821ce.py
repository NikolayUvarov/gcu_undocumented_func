#!/usr/bin/env python3
"""Fetch/check pinned RTL8821CE firmware (rtw88_wifi, 550-DRV-0042) into the ignored personal firmware store."""

import argparse
import sys
from pathlib import Path

sys.dont_write_bytecode = True  # no __pycache__ beside the scripts
from rtl8852be import BASE, ensure_asset  # noqa: E402

STORE = Path(__file__).resolve().parents[2] / "proprietary/firmware/rtw88"
# The same pinned linux-firmware revision as the RTL8852BE's; WHENCE: "Redistributable. See LICENCE.rtlwifi_firmware.txt".
ASSETS = (
    ("LICENCE.rtlwifi_firmware.txt", "LICENCE.rtlwifi_firmware.txt", 2115,
     "a61351665b4f264f6c631364f85b907d8f8f41f8b369533ef4021765f9f3b62e"),
    ("rtw8821c_fw.bin", "rtw88/rtw8821c_fw.bin", 139472,
     "2ef409bc418549fcf294061dd0cae1fc22fd9da79b60524950b25de18732f3f0"),
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
        parser.exit(1, f"rtl8821ce firmware: {error}\n")
    print(f"Personal store only ({BASE}); no disk or image was changed.")


if __name__ == "__main__":
    main()
