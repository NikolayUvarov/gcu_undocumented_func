#!/usr/bin/env python3
"""Collect read-only Linux RTL8168 evidence; exit 0: XID collected, 1: partial report, 2: refusal/error."""

import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import struct
import subprocess
import sys


SYSFS = Path("/sys/bus/pci/devices")
IDENTITY = (0x10EC, 0x8168)
SOURCE = "https://raw.githubusercontent.com/torvalds/linux/v6.12/drivers/net/ethernet/realtek/r8169_main.c"


class ProbeError(ValueError):
    pass


def pci_address(value):
    value = value.lower()
    if re.fullmatch(r"[0-9a-f]{2}:[0-9a-f]{2}\.[0-7]", value):
        value = "0000:" + value
    if not re.fullmatch(r"[0-9a-f]{4}:[0-9a-f]{2}:[0-9a-f]{2}\.[0-7]", value):
        raise ProbeError(f"Invalid PCI address: {value!r}; expected 0000:02:00.0")
    return value


def read_integer(path):
    try:
        return int(path.read_text().strip(), 0)
    except (OSError, ValueError) as error:
        raise ProbeError(f"Cannot read {path}: {error}") from error


def select_device(root, requested):
    if requested:
        device = root / pci_address(requested)
        identity = (read_integer(device / "vendor"), read_integer(device / "device"))
        if identity != IDENTITY:
            raise ProbeError(f"Refusing {device.name}: {identity[0]:04x}:{identity[1]:04x} is not 10ec:8168")
        return device
    if not root.is_dir():
        raise ProbeError(f"Linux PCI sysfs is unavailable: {root}")
    matches = []
    for device in sorted(root.iterdir()):
        try:
            if (read_integer(device / "vendor"), read_integer(device / "device")) == IDENTITY:
                matches.append(device)
        except ProbeError:
            continue
    if len(matches) != 1:
        found = ", ".join(device.name for device in matches) or "none"
        raise ProbeError(f"Expected one 10ec:8168 device, found {found}; use --pci to select an exact device")
    return matches[0]


def driver_info(text):
    return {key.strip(): value.strip() for line in text.splitlines() if ":" in line for key, value in [line.split(":", 1)]}


def decode_registers(data, driver, bus, expected_bus, byteorder):
    """Decode r8169's documented register layout from SOURCE; no chip initialization implementation is copied."""
    if driver != "r8169":
        raise ProbeError(f"Unknown register-dump layout for driver {driver!r}; expected Linux r8169")
    if pci_address(bus) != pci_address(expected_bus):
        raise ProbeError(f"ethtool bus {bus!r} does not match selected PCI device {expected_bus}")
    if byteorder != "little":
        raise ProbeError("Register decoding is supported only on a little-endian Linux host")
    if len(data) != 256:
        raise ProbeError(f"Expected exactly 256 r8169 register bytes, got {len(data)}")
    txconfig = struct.unpack_from("<I", data, 0x40)[0]
    if txconfig in (0, 0xFFFFFFFF):
        raise ProbeError(f"Invalid or inaccessible TxConfig: 0x{txconfig:08x}")
    xid = (txconfig >> 20) & 0xFCF
    if xid == 0:
        raise ProbeError("TxConfig has no usable MAC XID")
    return {"txconfig": f"0x{txconfig:08x}", "xid": f"0x{xid:03x}", "offset": "0x40", "byteorder": byteorder}


def query(argv):
    try:
        result = subprocess.run(argv, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=10, check=False)
        error = result.stderr.decode("utf-8", "replace").strip()
        if result.returncode:
            error = f"exit {result.returncode}: {error or 'no diagnostic'}"
            if "permi" in error.lower() or "not permitted" in error.lower():
                error += "; administrative read access may be required; rerun with authorized privileges"
        return result.stdout, {"argv": argv, "returncode": result.returncode, "stderr": error}
    except (OSError, subprocess.TimeoutExpired) as error:
        return b"", {"argv": argv, "returncode": None, "stderr": str(error)}


def write_private(path, data):
    with os.fdopen(os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600), "wb") as stream:
        stream.write(data)


def collect(device, output, ethtool):
    report = {
        "schema": "mind-rtl8168-probe-v1",
        "collected_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "host": platform.node(), "kernel": platform.release(), "architecture": platform.machine(),
        "pci": device.name, "read_only": True, "source": SOURCE,
        "identity": {}, "resources": [], "interfaces": [], "warnings": [],
    }
    warnings = report["warnings"]

    def read(path):
        try:
            return path.read_text().strip()
        except OSError as error:
            warnings.append(f"Cannot read {path}: {error}")
            return None

    for name in ("vendor", "device", "revision", "subsystem_vendor", "subsystem_device", "class", "irq"):
        report["identity"][name] = read(device / name)
    if (read_integer(device / "vendor"), read_integer(device / "device")) != IDENTITY:
        raise ProbeError("PCI device identity changed during collection; hardware queries refused")
    try:
        report["driver"] = (device / "driver").resolve(strict=True).name
    except OSError as error:
        report["driver"] = None
        warnings.append(f"No readable bound driver: {error}")
    resources = read(device / "resource")
    if resources is not None:
        write_private(output / "pci-resources.txt", (resources + "\n").encode())
        try:
            for index, line in enumerate(resources.splitlines()[:6]):
                start, end, flags = (int(word, 16) for word in line.split())
                report["resources"].append({"bar": index, "start": f"0x{start:x}", "end": f"0x{end:x}", "flags": f"0x{flags:x}", "size": end - start + 1 if flags and end >= start else 0})
        except ValueError:
            warnings.append("Unrecognized sysfs resource format; preserved raw resource text")
    try:
        interfaces = sorted((device / "net").iterdir())
    except OSError as error:
        interfaces = []
        warnings.append(f"No accessible network interface for this PCI device: {error}")
    if not interfaces:
        warnings.append("No interface to query; boot Linux with its normal driver already bound")
    if not ethtool:
        warnings.append("Missing ethtool; install the distribution's ethtool package and rerun")
    for interface in interfaces:
        name = interface.name
        row = {"name": name, "sysfs": {}, "commands": {}, "registers": None}
        report["interfaces"].append(row)
        for attr in ("address", "operstate", "carrier", "speed", "duplex", "mtu"):
            row["sysfs"][attr] = read(interface / attr)
        phy_id = interface / "phydev" / "phy_id"
        row["sysfs"]["phy_id"] = read(phy_id) if phy_id.exists() else None
        if not ethtool:
            continue
        info_data, info_command = query([ethtool, "-i", name])
        row["commands"]["driver"] = info_command
        write_private(output / f"{name}-driver.txt", info_data)
        info = driver_info(info_data.decode("utf-8", "replace"))
        row["driver_info"] = info
        link_data, link_command = query([ethtool, name])
        row["commands"]["link"] = link_command
        write_private(output / f"{name}-link.txt", link_data)
        if info_command["returncode"] != 0 or link_command["returncode"] != 0:
            warnings.append(f"{name}: ethtool information query failed; see commands and text files")
        try:
            if info_command["returncode"] != 0 or report["driver"] != "r8169" or info.get("driver") != "r8169":
                raise ProbeError("No verified Linux r8169 driver; register dump skipped")
            if pci_address(info.get("bus-info", "")) != pci_address(device.name):
                raise ProbeError("ethtool reports a different PCI device; register dump skipped")
            raw, command = query([ethtool, "-d", name, "raw", "on"])
            row["commands"]["register_dump"] = command
            if command["returncode"] != 0:
                raise ProbeError(f"Register dump failed: {command['stderr']}")
            filename = f"{name}-registers.bin"
            write_private(output / filename, raw)
            row["register_dump"] = {"file": filename, "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()}
            row["registers"] = decode_registers(raw, info["driver"], info["bus-info"], device.name, sys.byteorder)
        except ProbeError as error:
            warnings.append(f"{name}: {error}")
    report["status"] = "xid-collected" if any(row["registers"] is not None for row in report["interfaces"]) else "partial"
    write_private(output / "report.json", (json.dumps(report, indent=2) + "\n").encode())
    return report


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pci", help="PCI function, e.g. 0000:02:00.0; default: sole 10ec:8168 device")
    parser.add_argument("--output", type=Path, required=True, help="new directory for private evidence; existing directories are refused")
    args = parser.parse_args(argv)
    try:
        if sys.platform != "linux":
            raise ProbeError("This collector requires a Linux boot of the target machine")
        device = select_device(SYSFS, args.pci)
        args.output.mkdir(mode=0o700, parents=False, exist_ok=False)
        report = collect(device, args.output, shutil.which("ethtool"))
    except (OSError, ProbeError) as error:
        print(f"rtl8168-probe: {error}", file=sys.stderr)
        return 2
    print(f"{report['pci']}: {report['status']}; report: {args.output.resolve() / 'report.json'}")
    for row in report["interfaces"]:
        if row["registers"]:
            print(f"{row['name']}: TxConfig {row['registers']['txconfig']}, XID {row['registers']['xid']}")
    for warning in report["warnings"]:
        print(f"warning: {warning}", file=sys.stderr)
    return 0 if report["status"] == "xid-collected" else 1


if __name__ == "__main__":
    raise SystemExit(main())
