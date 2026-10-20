#!/usr/bin/env python3
"""Fetch/check pinned RTL8852BE firmware into the ignored personal firmware store."""

import argparse
import hashlib
import os
from pathlib import Path
import tempfile
import urllib.request

REVISION = "599764611a8ac213c6aa6dad17c941c2f46b53cb"
BASE = f"https://gitlab.com/kernel-firmware/linux-firmware/-/raw/{REVISION}/"
STORE = Path(__file__).resolve().parents[2] / "proprietary/firmware/rtw89"
ASSETS = (
    ("LICENCE.rtlwifi_firmware.txt", "LICENCE.rtlwifi_firmware.txt", 2115,
     "a61351665b4f264f6c631364f85b907d8f8f41f8b369533ef4021765f9f3b62e"),
    ("rtw8852b_fw-1.bin", "rtw89/rtw8852b_fw-1.bin", 1245944,
     "5b68415e3bfe72715d63a70703d4471b04b5475b8ff69cfdc8cdf48233cd5d3a"),
)


def verify(data, size, digest):
    if len(data) != size:
        raise ValueError(f"size mismatch: expected {size}, got {len(data)}")
    if hashlib.sha256(data).hexdigest() != digest:
        raise ValueError("SHA-256 mismatch")


def cached(path, size, digest):
    with path.open("rb") as source:
        verify(source.read(size + 1), size, digest)


def ensure_asset(directory, asset, fetch):
    name, upstream, size, digest = asset
    path = directory / name
    try:
        cached(path, size, digest)
        return path
    except (FileNotFoundError, ValueError):
        if not fetch:
            raise
    with urllib.request.urlopen(BASE + upstream, timeout=30) as response:
        data = response.read(size + 1)
    verify(data, size, digest)
    directory.mkdir(parents=True, exist_ok=True)
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(dir=directory, prefix=name + ".", delete=False) as output:
            temporary = Path(output.name)
            output.write(data)
            output.flush()
            os.fsync(output.fileno())
        os.replace(temporary, path)
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)
    return path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("fetch", "check"))
    args = parser.parse_args()
    try:
        for asset in ASSETS:
            path = ensure_asset(STORE, asset, args.action == "fetch")
            print(f"verified {asset[3]}  {path}")
    except (OSError, ValueError) as error:
        parser.exit(1, f"rtl8852be firmware: {error}\n")
    print("Personal store only; no disk or image was changed.")


if __name__ == "__main__":
    main()
