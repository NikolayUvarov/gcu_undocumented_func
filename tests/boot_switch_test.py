"""boot_switch.py against a model of the firmware's boot variables and of lsblk: no disk or variable is touched."""
import copy
import json
from pathlib import Path
import struct
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from scripts import boot_switch  # noqa: E402
from scripts.boot_switch import Refused, entry_for, mbr_identity, parse_entries  # noqa: E402

SIGNATURE, START, SIZE = 0xBE1AFDFA, 0x3F, 0xFBFC1
UBUNTU = r"HD(1,GPT,6c1e2c4f-0d9a-4bd2-9b44-3c1d5b0f5a11,0x800,0x100000)/File(\EFI\ubuntu\shimx64.efi)"
MIND = f"PciRoot(0x0)/Pci(0x8,0x1)/Pci(0x0,0x3)/USB(3,0)/HD(1,MBR,0x{SIGNATURE:x},0x{START:x},0x{SIZE:x})"


def mbr(signature=SIGNATURE, start=START, size=SIZE):
    sector = bytearray(512)
    struct.pack_into("<I", sector, 440, signature)
    struct.pack_into("<II", sector, 454, start, size)
    sector[510:] = b"\x55\xaa"
    return bytes(sector)


def stick(path="/dev/sdb", serial="AA00000000000123", mounted=None):
    return {"path": path, "type": "disk", "model": "Flash Disk", "serial": serial, "mountpoints": [None],
            "children": [{"path": path + "1", "type": "part", "label": "MIND CORE", "mountpoints": [mounted]},
                         {"path": path + "2", "type": "part", "label": "MIND LOG", "mountpoints": [None]}]}


def system_disk():
    return {"path": "/dev/nvme0n1", "type": "disk", "model": "NVMe", "mountpoints": [None],
            "children": [{"path": "/dev/nvme0n1p1", "type": "part", "label": None, "mountpoints": ["/boot/efi"]},
                         {"path": "/dev/nvme0n1p2", "type": "part", "label": None, "mountpoints": ["/"]}]}


class Firmware:
    """The boot variables as efibootmgr shows and changes them, and the machine's disks."""

    def __init__(self, entries=None, order=("0001", "0000"), current="0001", tree=None, logs=(1, 2)):
        self.entries = entries if entries is not None else {
            "0000": {"active": True, "label": "Windows Boot Manager", "path": r"HD(1,GPT,…)/File(\EFI\Microsoft\Boot\bootmgfw.efi)"},
            "0001": {"active": True, "label": "ubuntu", "path": UBUNTU}}
        self.order, self.current, self.next = list(order), current, None
        self.disks = tree if tree is not None else [system_disk(), stick()]
        self.logs, self.calls, self.rebooted, self.mounted = list(logs), [], False, []
        self.create_changes_order = False

    def efibootmgr(self, *args):
        self.calls.append(args)
        if args == ("-v",):
            lines = [f"BootCurrent: {self.current}"] if self.current else []
            lines += [f"BootNext: {self.next}"] if self.next else []
            lines += ["Timeout: 1 seconds", "BootOrder: " + ",".join(self.order)]
            lines += [f"Boot{n}{'*' if e['active'] else ' '} {e['label']}\t{e['path']}" for n, e in sorted(self.entries.items())]
            return "\n".join(lines) + "\n"
        if args[0] == "--create-only":
            number = f"{max(int(n, 16) for n in self.entries) + 1:04X}"
            self.entries[number] = {"active": True, "label": args[args.index("--label") + 1],
                                    "path": f"HD(1,MBR,0x{SIGNATURE:x},0x{START:x},0x{SIZE:x})/File({args[args.index('--loader') + 1]})"}
            if self.create_changes_order:
                self.order.insert(0, number)
            return ""
        if args[0] == "--bootnext":
            self.next = args[1]
            return ""
        if args[0] == "--bootnum" and args[2] == "--active":
            self.entries[args[1]]["active"] = True
            return ""
        raise AssertionError(f"efibootmgr {args} is not expected")

    def tree(self):
        return copy.deepcopy(self.disks)

    def sector(self, path):
        return mbr()

    def secure_boot(self):
        return False

    def mount(self, device, directory):
        self.mounted.append(device)
        for n in self.logs:
            Path(directory, f"boot{n:04d}.log").write_text("[SHELL] AUTORUN /autorun.msh\n[SHELL] AUTORUN ENDED OK\n")
        Path(directory, "hw0001.txt").write_text("kernel\n")

    def umount(self, directory):
        pass

    def reboot(self):
        self.rebooted = True


class Parsing(unittest.TestCase):
    def test_entries_with_and_without_a_tab(self):
        boot = parse_entries("BootCurrent: 0001\nBootOrder: 0001,0003\n"
                             f"Boot0001* ubuntu\t{UBUNTU}\nBoot0003  UEFI: Flash Disk {MIND}\n      dp: 04 01 2a 00\n")
        self.assertEqual((boot["current"], boot["next"], boot["order"]), ("0001", None, ["0001", "0003"]))
        self.assertEqual(boot["entries"]["0001"], {"active": True, "label": "ubuntu", "path": UBUNTU})
        self.assertEqual(boot["entries"]["0003"], {"active": False, "label": "UEFI: Flash Disk", "path": MIND})

    def test_the_entry_of_the_disks_first_partition(self):
        entries = {"0001": {"path": UBUNTU}, "0003": {"path": MIND},
                   "0004": {"path": MIND + r"/File(\EFI\other.efi)"}}
        self.assertEqual(entry_for(entries, (SIGNATURE, START, SIZE)), "0003")
        self.assertIsNone(entry_for(entries, (SIGNATURE, START, SIZE + 1)))
        self.assertIsNone(entry_for({"0004": entries["0004"]}, (SIGNATURE, START, SIZE)))
        self.assertEqual(mbr_identity(mbr()), (SIGNATURE, START, SIZE))
        with self.assertRaises(Refused):
            mbr_identity(bytes(512))


class Switching(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.state = Path(self.directory.name, "state.json")
        self.saved, boot_switch.STATE = boot_switch.STATE, self.state
        self.addCleanup(setattr, boot_switch, "STATE", self.saved)

    def written(self, firmware):
        return [c for c in firmware.calls if c != ("-v",)]

    def test_an_entry_made_with_create_only_and_booted_once(self):
        firmware = Firmware()
        number = boot_switch.once(firmware, reboot=True, out=lambda *_: None)
        self.assertEqual(number, "0002")
        self.assertEqual(firmware.next, "0002")
        self.assertEqual(firmware.order, ["0001", "0000"])
        self.assertTrue(firmware.rebooted)
        create = self.written(firmware)[0]
        self.assertEqual(create[:5], ("--create-only", "--disk", "/dev/sdb", "--part", "1"))
        self.assertEqual(json.loads(self.state.read_text())["newest_log"], 2)
        self.assertEqual(firmware.mounted, ["/dev/sdb2"])

    def test_an_existing_entry_is_used_and_activated(self):
        firmware = Firmware()
        firmware.entries["0003"] = {"active": False, "label": "UEFI: Flash Disk", "path": MIND}
        self.assertEqual(boot_switch.once(firmware, out=lambda *_: None), "0003")
        self.assertEqual(self.written(firmware), [("--bootnum", "0003", "--active"), ("--bootnext", "0003")])
        self.assertFalse(firmware.rebooted)

    def test_boot_order_is_never_written(self):
        for firmware in (Firmware(), Firmware(entries={"0001": {"active": True, "label": "ubuntu", "path": UBUNTU},
                                                       "0005": {"active": True, "label": "x", "path": MIND}})):
            boot_switch.once(firmware, out=lambda *_: None)
            self.assertFalse(any("--bootorder" in c or "-o" in c for c in firmware.calls))

    def test_refusals_change_nothing(self):
        cases = {
            "Linux not first": Firmware(order=("0000", "0001")),
            "no BootCurrent": Firmware(current=None),
            "no MIND Core disk": Firmware(tree=[system_disk()]),
            "two MIND Core disks": Firmware(tree=[system_disk(), stick(), stick("/dev/sdc", "BB")]),
            "a mounted MIND Core disk": Firmware(tree=[system_disk(), stick(mounted="/media/MIND CORE")]),
            "the system disk named": Firmware(),
        }
        for name, firmware in cases.items():
            with self.subTest(name), self.assertRaises(Refused):
                boot_switch.once(firmware, "/dev/nvme0n1" if name == "the system disk named" else None, True, out=lambda *_: None)
            self.assertEqual(self.written(firmware), [], name)
            self.assertFalse(firmware.rebooted or firmware.next, name)

    def test_a_create_that_moved_boot_order_stops_before_boot_next(self):
        firmware = Firmware()
        firmware.create_changes_order = True
        with self.assertRaises(Refused):
            boot_switch.once(firmware, reboot=True, out=lambda *_: None)
        self.assertIsNone(firmware.next)
        self.assertFalse(firmware.rebooted)

    def test_logs_say_whether_mind_core_booted(self):
        firmware = Firmware(logs=(1, 2))
        boot_switch.once(firmware, out=lambda *_: None)
        firmware.logs.append(3)
        said = []
        destination = boot_switch.logs(firmware, self.directory.name, out=said.append)
        self.assertTrue((destination / "boot0003.log").exists())
        self.assertIn("MIND Core booted: boot0003.log; autorun in the last: ended", said)
        firmware.logs = [1, 2]
        said.clear()
        boot_switch.logs(firmware, self.directory.name + "/again", out=said.append)
        self.assertTrue(any(s.startswith("MIND Core did not boot") for s in said))

    def test_status_changes_nothing(self):
        firmware = Firmware()
        said = []
        boot_switch.status(firmware, out=said.append)
        self.assertEqual(self.written(firmware), [])
        self.assertIn("  Boot0001* ubuntu (Linux, booted now)", said)
        self.assertTrue(any(s.startswith("MIND Core disk: /dev/sdb") for s in said))


if __name__ == "__main__":
    unittest.main()
