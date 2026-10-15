#!/usr/bin/env python3
"""Collect read-only Linux RTL8852BE identity; exit 0: identified, 1: partial, 2: refusal/error."""

import argparse
import datetime
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import sys

SYSFS = Path("/sys/bus/pci/devices")
IDENTITY = (0x10EC, 0xB852)
DRIVER = "rtw89_8852be"
SOURCE_REVISION = "05f7e89ab9731565d8a62e3b5d1ec206485eeb0b"
SOURCE = f"https://github.com/torvalds/linux/tree/{SOURCE_REVISION}/drivers/net/wireless/realtek/rtw89"
BOOT_SOURCE = "https://github.com/torvalds/linux/blob/028ef9c96e96197026887c0f092424679298aae8/drivers/net/wireless/realtek/rtw89/core.c#L6535"
MAX_TEXT = 32768
CUT_MISSING = (
    "No unambiguous digital chip cut was found in the current-boot driver log. mac80211's ethtool "
    "register dump is empty; reading rtw89/btc_info changes firmware reporting. "
    "Collect SYS_CFG1 with the MIND read-only probe; do not infer the cut from PCI revision."
)


class ProbeError(ValueError):
    pass


def pci_address(value):
    value = value.lower()
    if re.fullmatch(r"[0-9a-f]{2}:[0-9a-f]{2}\.[0-7]", value):
        value = "0000:" + value
    if not re.fullmatch(r"[0-9a-f]{4}:[0-9a-f]{2}:[0-9a-f]{2}\.[0-7]", value):
        raise ProbeError("Invalid PCI address; expected 0000:03:00.0")
    return value


def read_text(path):
    with path.open("rb") as stream:
        data = stream.read(MAX_TEXT + 1)
    if len(data) > MAX_TEXT:
        raise ProbeError(f"Oversized text attribute: {path}")
    return data.decode("utf-8", "strict").strip()


def read_integer(path):
    try:
        return int(read_text(path), 0)
    except (OSError, ValueError) as error:
        raise ProbeError(f"Cannot read integer attribute {path}: {error}") from error


def select_device(root, requested):
    if requested:
        device = root / pci_address(requested)
        if (read_integer(device / "vendor"), read_integer(device / "device")) != IDENTITY:
            raise ProbeError(f"Refusing {device.name}: not 10ec:b852")
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
        raise ProbeError(f"Expected one 10ec:b852, found {found}; use --pci for an exact device")
    pci_address(matches[0].name)
    return matches[0]


def driver_info(text):
    allowed = {"driver", "version", "firmware-version", "bus-info"}
    result = {}
    for line in text.splitlines():
        key, separator, value = line.partition(":")
        if separator and key.strip() in allowed:
            if key.strip() in result:
                raise ProbeError("Duplicate ethtool identity field")
            result[key.strip()] = value.strip()
    return result


def log_pattern(bus):
    prefix = re.escape(f"{DRIVER} {pci_address(bus)}: ")
    return "^" + prefix + r"(chip rfe_type is [0-9]+|chip info CID: [0-9a-fA-F]+, CV: [0-9a-fA-F]+, AID: [0-9a-fA-F]+, ACV: [0-9a-fA-F]+, RFE: [0-9]+|Firmware version [0-9.]+ \([0-9a-fA-F]{8}\), cmd version [0-9]+, type [0-9]+)$"


def parse_boot_records(text, bus):
    """Retain only known numeric identity records from this device, never arbitrary log text."""
    if len(text.encode()) > MAX_TEXT:
        raise ProbeError("Oversized kernel identity query")
    prefix = re.escape(f"{DRIVER} {pci_address(bus)}: ")
    rfe_pattern = re.compile(prefix + r"chip rfe_type is ([0-9]{1,3})")
    chip_pattern = re.compile(prefix + r"chip info CID: ([0-9a-fA-F]{1,2}), CV: ([0-9a-fA-F]), AID: ([0-9a-fA-F]{1,4}), ACV: ([0-9a-fA-F]{1,2}), RFE: ([0-9]{1,3})")
    fw_pattern = re.compile(prefix + r"Firmware version ([0-9]{1,3})\.([0-9]{1,3})\.([0-9]{1,3})\.([0-9]{1,3}) \(([0-9a-fA-F]{8})\), cmd version ([0-9]{1,3}), type ([0-9]{1,3})")
    records = []
    for line in text.splitlines():
        line = line.strip()
        rfe = rfe_pattern.fullmatch(line)
        chip = chip_pattern.fullmatch(line)
        fw = fw_pattern.fullmatch(line)
        record = None
        if rfe and int(rfe[1]) <= 255:
            record = {"kind": "rfe_type", "value": int(rfe[1])}
        elif chip and int(chip[5]) <= 255:
            record = {"kind": "chip_info", "cid": int(chip[1], 16), "chip_cut": int(chip[2], 16),
                      "aid": int(chip[3], 16), "analog_cut": int(chip[4], 16), "rfe_type": int(chip[5])}
        elif fw and all(int(fw[n]) <= 255 for n in (1, 2, 3, 4, 6, 7)):
            record = {"kind": "firmware", "version": ".".join(fw.group(n) for n in (1, 2, 3, 4)),
                      "commit": fw[5].lower(), "command_version": int(fw[6]), "type": int(fw[7])}
        if record is not None and record not in records:
            records.append(record)
    return records


def query(argv):
    try:
        completed = subprocess.run(argv, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                                   stderr=subprocess.PIPE, timeout=10, check=False)
        error = completed.stderr[:4096].decode("utf-8", "replace").strip()
        if completed.returncode:
            error = f"exit {completed.returncode}: {error or 'no diagnostic'}"
            if "permi" in error.lower():
                error += "; authorized administrative read access may be required"
        if len(completed.stdout) > MAX_TEXT:
            return b"", {"argv": argv, "returncode": None, "stderr": "Oversized command output refused"}
        return completed.stdout, {"argv": argv, "returncode": completed.returncode, "stderr": error}
    except (OSError, subprocess.TimeoutExpired) as error:
        return b"", {"argv": argv, "returncode": None, "stderr": str(error)}


def write_private(path, data):
    with os.fdopen(os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600), "wb") as stream:
        stream.write(data)


def collect(device, output, ethtool, journalctl):
    report = {
        "schema": "mind-rtl8852be-probe-v1", "read_only": True,
        "collected_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "host": platform.node(), "kernel": platform.release(), "architecture": platform.machine(),
        "pci": device.name, "source": SOURCE, "boot_record_source": BOOT_SOURCE, "identity": {}, "interfaces": [],
        "commands": {}, "boot_identity_records": [], "rfe_type": None,
        "chip_cut": None, "chip_cut_source": None, "chip_cut_missing_reason": CUT_MISSING, "warnings": [],
    }
    warnings = report["warnings"]

    def read(path):
        try:
            return read_text(path)
        except (OSError, ValueError) as error:
            warnings.append(f"Cannot read {path}: {error}")
            return None

    for attr in ("vendor", "device", "revision", "subsystem_vendor", "subsystem_device", "class", "irq"):
        report["identity"][attr] = read(device / attr)
    if (read_integer(device / "vendor"), read_integer(device / "device")) != IDENTITY:
        raise ProbeError("PCI identity changed during collection; hardware queries refused")
    report["pci_resources"] = read(device / "resource")
    try:
        report["driver"] = (device / "driver").resolve(strict=True).name
    except OSError as error:
        report["driver"] = None
        warnings.append(f"No readable bound driver: {error}")
    try:
        interfaces = sorted((device / "net").iterdir())
    except OSError:
        interfaces = []
    if len(interfaces) > 16:
        raise ProbeError("Too many interfaces for one RTL8852BE; collection refused")
    if not interfaces:
        warnings.append("No network interface; the collector does not load or bind drivers")
    for interface in interfaces:
        name = interface.name
        if not re.fullmatch(r"[A-Za-z0-9_.:-]{1,15}", name) or name.startswith("-"):
            warnings.append("Unsupported interface name; query skipped")
            continue
        row = {"name": name, "sysfs": {key: read(interface / key) for key in ("operstate", "carrier", "mtu")}}
        report["interfaces"].append(row)
        if not ethtool or report["driver"] != DRIVER:
            continue
        data, command = query([ethtool, "-i", name])
        row["command"] = command
        try:
            info = driver_info(data.decode("utf-8", "replace"))
            if command["returncode"] != 0 or info.get("driver") != DRIVER:
                raise ProbeError("No verified rtw89_8852be ethtool identity")
            if pci_address(info.get("bus-info", "")) != pci_address(device.name):
                raise ProbeError("ethtool bus identity does not match the selected PCI device")
            row["driver_info"] = info
        except ProbeError as error:
            warnings.append(f"{name}: {error}")
    if not ethtool:
        warnings.append("Missing ethtool; driver/firmware query unavailable")
    if report["driver"] != DRIVER:
        warnings.append("No verified bound rtw89_8852be; external queries skipped")
    elif not journalctl:
        warnings.append("Missing journalctl; existing boot RFE/firmware records unavailable")
    else:
        argv = [journalctl, "-k", "--boot=0", "--no-pager", "--output=cat", "--lines=32",
                "--grep=" + log_pattern(device.name)]
        data, command = query(argv)
        report["commands"]["boot_identity"] = command
        if command["returncode"] == 0:
            records = parse_boot_records(data.decode("utf-8", "replace"), device.name)
            report["boot_identity_records"] = records
            rfe = {row["value"] for row in records if row["kind"] == "rfe_type"}
            chip_records = [row for row in records if row["kind"] == "chip_info"]
            rfe.update(row["rfe_type"] for row in chip_records)
            if len(rfe) == 1:
                report["rfe_type"] = next(iter(rfe))
            elif len(rfe) > 1:
                warnings.append("Conflicting RFE records in this boot; no RFE value selected")
            if len(chip_records) == 1 and len(rfe) == 1:
                report["chip_cut"] = chip_records[0]["chip_cut"]
                report["chip_cut_source"] = "current-boot-driver-log"
                report["chip_cut_missing_reason"] = None
            elif len(chip_records) > 1:
                warnings.append("Conflicting chip identity records in this boot; no chip cut selected")
        else:
            warnings.append("No accessible matching boot identity records; see boot_identity command diagnostic")
    if report["rfe_type"] is None:
        warnings.append("RFE type not established; it is not inferred from PCI/subsystem identity")
    if report["chip_cut"] is None:
        warnings.append(CUT_MISSING)
    report["status"] = "partial" if warnings else "identity-collected"
    write_private(output / "report.json", (json.dumps(report, indent=2) + "\n").encode())
    return report


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pci", help="PCI function; default: sole 10ec:b852 device")
    parser.add_argument("--output", type=Path, required=True, help="new directory for private evidence")
    args = parser.parse_args(argv)
    try:
        if sys.platform != "linux":
            raise ProbeError("This collector requires Linux on the target machine")
        device = select_device(SYSFS, args.pci)
        args.output.mkdir(mode=0o700, parents=False, exist_ok=False)
        report = collect(device, args.output, shutil.which("ethtool"), shutil.which("journalctl"))
    except (OSError, ProbeError) as error:
        print(f"rtl8852be-probe: {error}", file=sys.stderr)
        return 2
    print(f"{report['pci']}: {report['status']}; report: {args.output.resolve() / 'report.json'}")
    for warning in report["warnings"]:
        print(f"warning: {warning}", file=sys.stderr)
    return 0 if report["status"] == "identity-collected" else 1


if __name__ == "__main__":
    raise SystemExit(main())
