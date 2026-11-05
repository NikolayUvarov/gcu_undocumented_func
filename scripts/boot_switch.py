#!/usr/bin/env python3
"""One boot of MIND Core from Linux by the firmware's BootNext, and that boot's logs read back (000-KRN-0124).

  boot_switch.py status                       the boot entries, Linux's and MIND Core's, the MIND Core disk
  boot_switch.py once [--disk DEV] [--reboot] BootNext set to the MIND Core disk's entry (made if missing)
  boot_switch.py logs DIR [--disk DEV]        the MIND LOG volume copied into DIR, read-only, and how far the
                                              last boot got by the loader's mark (000-KRN-0211)

BootOrder is never written: Linux's entry stays first, so whatever ends the run (reboot, a panic, the watchdog, a power
cut, a bootloader that refuses the volume) the next boot is Linux. Run as root, on a machine booted by UEFI.
"""
import argparse
import json
import os
from pathlib import Path
import re
import shutil
import struct
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, str(Path(__file__).resolve().parent))
from write_usb_linux import descendants, devices  # noqa: E402

LOADER = r"\EFI\BOOT\BOOTX64.EFI"
LABEL = "MIND Core"
BOOT_VOLUME, LOG_VOLUME = "MIND CORE", "MIND LOG"
STATE = Path(os.environ.get("MIND_BOOT_SWITCH_STATE", "/var/tmp/mind-boot-switch.json"))
SECURE_BOOT = Path("/sys/firmware/efi/efivars/SecureBoot-8be4df61-93ca-11d2-aa0d-00e098032b8c")
LOG_NAME = re.compile(r"boot(\d{4})\.log$", re.IGNORECASE)
# The loader's mark on the boot volume (bootloader/src/mark.rs): the boot count and the last point it reached.
MARK = Path("MIND", "REACHED")
STAGES = {
    "STARTED": "the loader started and stopped before it had read and checked the kernel and the services",
    "READ": "the loader read and checked the kernel and the services, and stopped before boot services ended "
            "(choosing the display, or starting the processors)",
    "HANDOVER": "the loader handed over to the kernel: the boot stopped in the kernel, init or the services "
                "before the log volume's first write",
}
DEVICE_PATH = r"\s+(?=(?:HD|PciRoot|VenHw|VenMedia|BBS|File|Fv|FvFile|MemoryMapped|Acpi|USB|Uri)\()"


class Refused(Exception):
    """A reason not to go on, said to the user."""


def parse_entries(text):
    """`efibootmgr -v`'s output: BootCurrent, BootNext, BootOrder and each entry's active flag, label and path."""
    boot = {"current": None, "next": None, "order": [], "entries": {}}
    for line in text.splitlines():
        if m := re.match(r"BootCurrent:\s*([0-9A-Fa-f]{4})", line):
            boot["current"] = m[1].upper()
        elif m := re.match(r"BootNext:\s*([0-9A-Fa-f]{4})", line):
            boot["next"] = m[1].upper()
        elif m := re.match(r"BootOrder:\s*(.*)", line):
            boot["order"] = [n.upper() for n in m[1].strip().split(",") if n]
        elif m := re.match(r"Boot([0-9A-Fa-f]{4})(\*?)\s+(.*)", line):
            # The label ends at a tab, or where the device path starts in versions that print none.
            label, path = m[3].split("\t", 1) if "\t" in m[3] else (re.split(DEVICE_PATH, m[3], maxsplit=1) + [""])[:2]
            boot["entries"][m[1].upper()] = {"active": m[2] == "*", "label": label.strip(), "path": path.strip()}
    return boot


def mbr_identity(sector):
    """The disk's MBR signature and its first partition's start and size, as a firmware device path names them."""
    if len(sector) != 512 or sector[510:] != b"\x55\xaa":
        raise Refused("the disk has no MBR")
    signature = struct.unpack_from("<I", sector, 440)[0]
    start, size = struct.unpack_from("<II", sector, 454)
    return signature, start, size


def entry_for(entries, identity):
    """The entry that starts the disk's first partition: HD(1,MBR,signature,start,size), with our loader or none."""
    signature, start, size = identity
    for number, entry in sorted(entries.items()):
        hd = re.search(r"HD\(1,MBR,0x([0-9a-f]+),0x([0-9a-f]+),0x([0-9a-f]+)\)", entry["path"], re.IGNORECASE)
        loader = re.search(r"File\(([^)]*)\)", entry["path"])
        if hd and (int(hd[1], 16), int(hd[2], 16), int(hd[3], 16)) == (signature, start, size) \
                and (loader is None or loader[1].upper() == LOADER):
            return number
    return None


def mind_disks(tree):
    """Disks whose first partition is the MIND CORE volume and whose second is MIND LOG."""
    found = []
    for root in tree:
        if root.get("type") != "disk":
            continue
        parts = [n for n in descendants(root) if n is not root and n.get("type") == "part"]
        if len(parts) >= 2 and parts[0].get("label") == BOOT_VOLUME and parts[1].get("label") == LOG_VOLUME:
            found.append((root, parts[0], parts[1]))
    return found


def choose_disk(tree, requested=None):
    """The one MIND Core disk, or the one named; refused if mounted, ambiguous or missing."""
    found = mind_disks(tree)
    if requested:
        found = [d for d in found if d[0]["path"] == requested]
        if not found:
            raise Refused(f"{requested} has no {BOOT_VOLUME} volume followed by {LOG_VOLUME}")
    if not found:
        raise Refused(f"no disk with a {BOOT_VOLUME} volume followed by {LOG_VOLUME}: write the image first")
    if len(found) > 1:
        raise Refused("several MIND Core disks: name one with --disk (" + ", ".join(d[0]["path"] for d in found) + ")")
    disk = found[0]
    mounted = [n["path"] for n in descendants(disk[0]) if any(m for m in n.get("mountpoints") or [])]
    if mounted:
        raise Refused(f"{', '.join(mounted)} mounted: unmount the MIND Core disk first")
    return disk


def check_order(boot):
    """Linux, the entry this system booted from, must come first in BootOrder."""
    if boot["current"] is None:
        raise Refused("no BootCurrent: Linux was not started by UEFI")
    if not boot["order"] or boot["order"][0] != boot["current"]:
        raise Refused(f"Boot{boot['current']}, the entry Linux booted from, is not first in BootOrder "
                      f"({','.join(boot['order']) or 'empty'}); a restart after MIND Core would not come back here. "
                      f"Put it first in the firmware's setup, or with efibootmgr --bootorder {boot['current']},…")


class System:
    """What the script does to the machine; the tests replace it."""

    def efibootmgr(self, *args):
        return subprocess.run(["efibootmgr", *args], check=True, capture_output=True, text=True).stdout

    def tree(self):
        return devices()

    def sector(self, path):
        with open(path, "rb") as disk:
            return disk.read(512)

    def secure_boot(self):
        try:
            return SECURE_BOOT.read_bytes()[4:5] == b"\x01"
        except OSError:
            return None

    def mount(self, device, directory):
        subprocess.run(["mount", "-o", "ro", "-t", "vfat", device, directory], check=True)

    def umount(self, directory):
        subprocess.run(["umount", directory], check=True)

    def reboot(self):
        # In 5 s, from systemd: the restart outlives the caller's process group, as an Effector exec's (000-KRN-0124).
        if shutil.which("systemd-run"):
            subprocess.run(["systemd-run", "--on-active=5", "/bin/systemctl", "reboot"], check=True, capture_output=True)
        else:
            subprocess.run(["systemctl", "reboot"], check=True)


def parse_mark(data):
    """(count, stage) of the loader's mark, or None for one of another shape."""
    words = data.split(b"\n", 1)[0].split()
    if len(words) != 4 or words[:2] != [b"MIND-REACHED", b"1"] or not re.fullmatch(rb"[0-9]+", words[2]):
        return None
    stage = words[3].decode("ascii", "replace")
    return (int(words[2]), stage) if stage in STAGES and int(words[2]) < 1 << 32 else None


def read_mark(root):
    """The mark on a mounted boot volume, or None: no mark, or a loader older than the mark."""
    try:
        return parse_mark((Path(root) / MARK).read_bytes())
    except OSError:
        return None


def read_log_volume(system, device, visit):
    """Mounts a volume of the disk (the log volume, or the boot volume for the mark) read-only in a temporary
    directory for `visit`, then unmounts it."""
    with tempfile.TemporaryDirectory(prefix="mind-log-") as directory:
        system.mount(device, directory)
        try:
            return visit(Path(directory))
        finally:
            system.umount(directory)


def newest_log(root):
    numbers = [int(m[1]) for p in root.iterdir() if (m := LOG_NAME.match(p.name))]
    return max(numbers, default=0)


def status(system, out=print):
    boot = parse_entries(system.efibootmgr("-v"))
    out(f"BootCurrent {boot['current'] or '-'}, BootNext {boot['next'] or '-'}, BootOrder {','.join(boot['order']) or '-'}")
    for number, entry in sorted(boot["entries"].items()):
        mark = " (Linux, booted now)" if number == boot["current"] else ""
        out(f"  Boot{number}{'*' if entry['active'] else ' '} {entry['label']}{mark}")
    try:
        disk, _, _ = choose_disk(system.tree())
        number = entry_for(boot["entries"], mbr_identity(system.sector(disk["path"])))
        out(f"MIND Core disk: {disk['path']} ({disk.get('model') or '?'}), entry "
            f"{'Boot' + number if number else 'none yet (once makes one)'}")
    except Refused as reason:
        out(f"MIND Core disk: {reason}")
    if system.secure_boot():
        out("Secure Boot is on: MIND Core starts only if its bootloader is signed with a key the firmware trusts")
    return boot


def once(system, requested=None, reboot=False, out=print):
    """BootNext set to the MIND Core disk's entry; BootOrder left as it is."""
    boot = parse_entries(system.efibootmgr("-v"))
    check_order(boot)
    disk, part, log = choose_disk(system.tree(), requested)
    identity = mbr_identity(system.sector(disk["path"]))
    number = entry_for(boot["entries"], identity)
    if number is None:
        partition = re.search(r"(\d+)$", part["path"])
        system.efibootmgr("--create-only", "--disk", disk["path"], "--part", partition[1] if partition else "1",
                          "--loader", LOADER, "--label", LABEL)
        after = parse_entries(system.efibootmgr("-v"))
        if after["order"] != boot["order"]:
            raise Refused(f"efibootmgr changed BootOrder to {','.join(after['order'])}; put back "
                          f"{','.join(boot['order'])} with efibootmgr --bootorder before switching")
        number = entry_for(after["entries"], identity)
        if number is None:
            raise Refused("the new boot entry does not name the disk's first partition")
        out(f"made Boot{number} {LABEL} for {disk['path']}")
        boot = after
    if not boot["entries"][number]["active"]:
        system.efibootmgr("--bootnum", number, "--active")
    newest = read_log_volume(system, log["path"], newest_log)
    mark = read_log_volume(system, part["path"], read_mark)
    system.efibootmgr("--bootnext", number)
    if parse_entries(system.efibootmgr("-v"))["next"] != number:
        raise Refused("the firmware did not keep BootNext")
    STATE.write_text(json.dumps({"disk": disk["path"], "serial": disk.get("serial"), "entry": number,
                                 "newest_log": newest, "mark": mark[0] if mark else None, "time": int(time.time())}))
    out(f"the next boot starts Boot{number} ({disk['path']}) once; every boot after it starts Boot{boot['order'][0]}")
    if system.secure_boot():
        out("Secure Boot is on: if the firmware refuses MIND Core's bootloader, it starts Linux instead")
    out("With the firmware's Fast Boot on, a firmware may skip the stick and BootNext and start Linux (the Ryzen did): "
        "turn Fast Boot off in its setup")
    if reboot:
        out("restarting in 5 s")
        system.reboot()
    return number


def reached(before, mark):
    """What the loader's mark says of the boots since `once` recorded `before`, for a run that left no new log."""
    if mark is None:
        return "the loader left no mark: it never started, or it is older than the mark (000-KRN-0211)"
    count, stage = mark
    if before is None:
        return f"the loader's mark is boot {count}, {stage}; no mark was recorded at the switch to compare it with"
    if count == before:
        return (f"the loader's mark is still boot {count}: the loader did not start. The firmware may skip USB "
                "(Fast Boot), ignore BootNext, or stop before it starts the stick's loader")
    if count < before:
        return f"the loader's mark went back from boot {before} to {count}: the stick was written again since"
    since = f" ({count - before} boots since the switch; the mark is the last one's)" if count - before > 1 else ""
    return f"boot {count}: {STAGES[stage]}{since}"


def logs(system, target, requested=None, out=print):
    """The log volume copied into a new directory under `target`; says whether MIND Core booted since `once`, and
    from the loader's mark how far a boot that left no log got."""
    _, part, log = choose_disk(system.tree(), requested)
    try:
        state = json.loads(STATE.read_text())
        before, before_mark = state["newest_log"], state.get("mark")
    except (OSError, ValueError, KeyError, TypeError):
        before, before_mark = None, None
    destination = Path(target) / time.strftime("mind-log-%Y%m%d-%H%M%S")

    def copy(root):
        shutil.copytree(root, destination)
        return newest_log(root)

    newest = read_log_volume(system, log["path"], copy)
    mark = read_log_volume(system, part["path"], read_mark)
    out(f"copied {log['path']} into {destination}")
    if before is None:
        out(f"the newest boot log is boot{newest:04d}.log (no switch was recorded)")
    elif newest > before:
        boots = ", ".join(f"boot{n:04d}.log" for n in range(before + 1, newest + 1))
        text = (destination / next(p.name for p in destination.iterdir() if LOG_NAME.match(p.name)
                                   and int(LOG_NAME.match(p.name)[1]) == newest)).read_text(errors="replace")
        autorun = "ended" if "AUTORUN ENDED" in text else "started, no end logged" if "AUTORUN" in text else "none"
        out(f"MIND Core booted: {boots}; autorun in the last: {autorun}")
        if mark:
            out(f"the loader's mark: boot {mark[0]}, {mark[1]}")
    else:
        out(f"MIND Core did not boot: no new boot log; {reached(before_mark, mark)}")
    return destination


def main(argv=None, system=None):
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("status", help="the boot entries and the MIND Core disk; changes nothing")
    switch = commands.add_parser("once", help="BootNext to the MIND Core disk (an entry made if missing)")
    switch.add_argument("--disk", help="the MIND Core disk, e.g. /dev/sdb, when there are several")
    switch.add_argument("--reboot", action="store_true", help="restart the machine now")
    read = commands.add_parser("logs", help="copy the MIND LOG volume into DIR, read-only")
    read.add_argument("dir")
    read.add_argument("--disk")
    args = parser.parse_args(argv)
    system = system or System()
    try:
        if args.command == "status":
            status(system)
        elif args.command == "once":
            once(system, args.disk, args.reboot)
        else:
            logs(system, args.dir, args.disk)
    except Refused as reason:
        print(f"boot_switch: refused: {reason}", file=sys.stderr)
        return 1
    except subprocess.CalledProcessError as error:
        print(f"boot_switch: {' '.join(error.cmd)} failed ({error.returncode}): {(error.stderr or '').strip()}",
              file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
