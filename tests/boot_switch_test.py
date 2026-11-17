"""boot_switch.py against a model of the firmware's boot variables and of lsblk: no disk or variable is touched."""
import copy
import json
from pathlib import Path
import struct
import sys
import tempfile
import unittest
import uuid

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from scripts import boot_switch  # noqa: E402
from scripts.boot_switch import Refused, entry_for, identity, parse_entries  # noqa: E402

SIGNATURE, START, SIZE = 0xBE1AFDFA, 0x3F, 0xFBFC1
UBUNTU = r"HD(1,GPT,6c1e2c4f-0d9a-4bd2-9b44-3c1d5b0f5a11,0x800,0x100000)/File(\EFI\ubuntu\shimx64.efi)"
MIND = f"PciRoot(0x0)/Pci(0x8,0x1)/Pci(0x0,0x3)/USB(3,0)/HD(1,MBR,0x{SIGNATURE:x},0x{START:x},0x{SIZE:x})"


X86, ARM = r"\EFI\BOOT\BOOTX64.EFI", r"\EFI\BOOT\BOOTAA64.EFI"
# Ubuntu's disk on the ARM computer (000-KRN-0257): its ESP and root, then MIND CORE and MIND LOG at 3 and 4.
PARTUUID, GPT_START, GPT_SIZE = "5d0b8c1e-2f3a-4b6c-9d8e-7f1a2b3c4d5e", 0x3A38000, 0x200000
GPT_HD = f"HD(3,GPT,{PARTUUID},0x{GPT_START:x},0x{GPT_SIZE:x})"


def mbr(signature=SIGNATURE, start=START, size=SIZE, number=1):
    sector = bytearray(512)
    struct.pack_into("<I", sector, 440, signature)
    sector[446 + 16 * (number - 1) + 4] = 0x0C
    struct.pack_into("<II", sector, 446 + 16 * (number - 1) + 8, start, size)
    sector[510:] = b"\x55\xaa"
    return bytes(sector)


def gpt(parts, block=512):
    """A GPT disk's first sectors: the protective MBR, the header in LBA 1 and the entries from LBA 2, each part
    (number, PARTUUID, first, size)."""
    image = bytearray(block * 2 + 128 * 128)
    struct.pack_into("<B3sB3sII", image, 446, 0, b"\0\2\0", 0xEE, b"\xff\xff\xff", 1, 0xFFFFFFFF)
    image[510:512] = b"\x55\xaa"
    image[block:block + 92] = struct.pack("<8sIIIIQQQQ16sQIII", b"EFI PART", 0x10000, 92, 0, 0, 1, 0, 34, 0, bytes(16), 2, 128, 128, 0)
    for number, partuuid, first, size in parts:
        at = block * 2 + (number - 1) * 128
        image[at:at + 48] = uuid.uuid4().bytes_le + uuid.UUID(partuuid).bytes_le + struct.pack("<QQ", first, first + size - 1)
    return bytes(image)


def ubuntu_disk(mounted_mind=None):
    """Ubuntu's own disk, GPT: its ESP and root mounted, MIND CORE and MIND LOG beside them."""
    return {"path": "/dev/sda", "type": "disk", "model": "Samsung SSD 860", "log-sec": 512, "mountpoints": [None],
            "children": [{"path": "/dev/sda1", "type": "part", "label": None, "mountpoints": ["/boot/efi"]},
                         {"path": "/dev/sda2", "type": "part", "label": None, "mountpoints": ["/"]},
                         {"path": "/dev/sda3", "type": "part", "label": "MIND CORE", "mountpoints": [mounted_mind]},
                         {"path": "/dev/sda4", "type": "part", "label": "MIND LOG", "mountpoints": [None]}]}


UBUNTU_GPT = gpt([(1, str(uuid.uuid4()), 0x800, 0x100000), (2, str(uuid.uuid4()), 0x100800, 0x3937800),
                  (3, PARTUUID, GPT_START, GPT_SIZE), (4, str(uuid.uuid4()), GPT_START + GPT_SIZE, 0x80000)])


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

    def __init__(self, entries=None, order=("0001", "0000"), current="0001", tree=None, logs=(1, 2), machine="x86_64", images=None, made=None):
        self.entries = entries if entries is not None else {
            "0000": {"active": True, "label": "Windows Boot Manager", "path": r"HD(1,GPT,…)/File(\EFI\Microsoft\Boot\bootmgfw.efi)"},
            "0001": {"active": True, "label": "ubuntu", "path": UBUNTU}}
        self.order, self.current, self.next = list(order), current, None
        self.disks = tree if tree is not None else [system_disk(), stick()]
        self.logs, self.calls, self.rebooted, self.mounted = list(logs), [], False, []
        # The loader's mark on the boot volume (000-KRN-0211): (count, stage), or None for a loader without one.
        self.mark = (5, "HANDOVER")
        self.create_changes_order = False
        # What each disk's first sectors hold, the machine efibootmgr runs on, and the HD() of an entry it makes.
        self.images, self.arch = images or {}, machine
        self.made = made or f"HD(1,MBR,0x{SIGNATURE:x},0x{START:x},0x{SIZE:x})"

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
                                    "path": f"{self.made}/File({args[args.index('--loader') + 1]})"}
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

    def read(self, path, offset, size):
        return self.images.get(path, mbr())[offset:offset + size]

    def machine(self):
        return self.arch

    def secure_boot(self):
        return False

    def mount(self, device, directory):
        self.mounted.append(device)
        if device.endswith(("sdb1", "sda3", "sdc2")):
            if self.mark:
                Path(directory, "MIND").mkdir()
                Path(directory, "MIND", "REACHED").write_bytes(f"MIND-REACHED 1 {self.mark[0]} {self.mark[1]}".ljust(47).encode() + b"\n")
            return
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
        ident = ("MBR", 1, SIGNATURE, START, SIZE)
        self.assertEqual(entry_for(entries, ident, X86), "0003")
        self.assertIsNone(entry_for(entries, ("MBR", 1, SIGNATURE, START, SIZE + 1), X86))
        self.assertIsNone(entry_for(entries, ("MBR", 2, SIGNATURE, START, SIZE), X86))
        self.assertIsNone(entry_for({"0004": entries["0004"]}, ident, X86))
        firmware = Firmware()
        self.assertEqual(identity(firmware, stick(), stick()["children"][0]), ident)
        firmware.images["/dev/sdb"] = bytes(512)
        with self.assertRaises(Refused):
            identity(firmware, stick(), stick()["children"][0])

    def test_a_gpt_partition_read_from_its_table(self):
        disk = ubuntu_disk()
        firmware = Firmware(images={"/dev/sda": UBUNTU_GPT})
        self.assertEqual(identity(firmware, disk, disk["children"][2]), ("GPT", 3, PARTUUID, GPT_START, GPT_SIZE))
        # 4096-byte logical sectors: the header in LBA 1 at 4096, the entries counted in those sectors too.
        disk["log-sec"] = 4096
        firmware.images["/dev/sda"] = gpt([(3, PARTUUID, GPT_START // 8, GPT_SIZE // 8)], block=4096)
        self.assertEqual(identity(firmware, disk, disk["children"][2]), ("GPT", 3, PARTUUID, GPT_START // 8, GPT_SIZE // 8))
        for path in ("/dev/sda5", "/dev/sda2"):  # past the entries the model holds; an unused entry
            with self.subTest(path), self.assertRaises(Refused):
                identity(Firmware(images={"/dev/sda": gpt([(3, PARTUUID, GPT_START, GPT_SIZE)])}), ubuntu_disk(), {"path": path})
        entries = {"0001": {"path": UBUNTU}, "0004": {"path": GPT_HD + rf"/File({X86})"}, "0005": {"path": GPT_HD.upper() + rf"/File({ARM})"}}
        self.assertEqual(entry_for(entries, ("GPT", 3, PARTUUID, GPT_START, GPT_SIZE), ARM), "0005")
        self.assertEqual(entry_for(entries, ("GPT", 3, PARTUUID, GPT_START, GPT_SIZE), X86), "0004")
        self.assertIsNone(entry_for(entries, ("GPT", 3, str(uuid.uuid4()), GPT_START, GPT_SIZE), ARM))


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
        self.assertEqual(json.loads(self.state.read_text())["mark"], 5)
        self.assertEqual(firmware.mounted, ["/dev/sdb2", "/dev/sdb1"])

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
        self.assertIn("the loader's mark: boot 5, HANDOVER", said)
        firmware.logs = [1, 2]
        said.clear()
        boot_switch.logs(firmware, self.directory.name + "/again", out=said.append)
        self.assertTrue(any(s.startswith("MIND Core did not boot") for s in said))

    def test_the_marks_stage_says_where_a_boot_without_a_log_stopped(self):
        firmware = Firmware(logs=(1, 2))
        boot_switch.once(firmware, out=lambda *_: None)
        cases = {(5, "HANDOVER"): "the loader did not start", (6, "STARTED"): "boot 6: the loader started and stopped before",
                 (6, "READ"): "boot 6: the loader read and checked", (6, "HANDOVER"): "boot 6: the loader handed over to the kernel",
                 (8, "HANDOVER"): "(3 boots since the switch", (2, "READ"): "went back from boot 5 to 2",
                 None: "the loader left no mark"}
        for n, (mark, expected) in enumerate(cases.items()):
            firmware.mark, said = mark, []
            boot_switch.logs(firmware, f"{self.directory.name}/{n}", out=said.append)
            line = next(s for s in said if s.startswith("MIND Core did not boot"))
            self.assertIn(expected, line, mark)

    def test_a_mark_of_another_shape_is_not_read(self):
        good = b"MIND-REACHED 1 7 READ".ljust(47) + b"\n"
        self.assertEqual(boot_switch.parse_mark(good), (7, "READ"))
        self.assertEqual(boot_switch.parse_mark(good + b"junk\n"), (7, "READ"))
        for bad in (b"", b"MIND-REACHED 2 7 READ\n", b"MIND-REACHED 1 +7 READ\n", b"MIND-REACHED 1 7 BOOTED\n",
                    b"MIND-REACHED 1 7 READ more\n", b"MIND-REACHED 1 4294967296 READ\n", bytes(48)):
            self.assertIsNone(boot_switch.parse_mark(bad), bad)

    def test_ubuntus_own_disk_on_aarch64(self):
        """000-KRN-0257: MIND CORE and MIND LOG at 3 and 4 of the GPT disk Linux runs from, its root mounted."""
        firmware = Firmware(tree=[ubuntu_disk()], machine="aarch64", images={"/dev/sda": UBUNTU_GPT}, made=GPT_HD)
        number = boot_switch.once(firmware, out=lambda *_: None)
        self.assertEqual(firmware.next, number)
        self.assertEqual(firmware.order, ["0001", "0000"])
        self.assertEqual(self.written(firmware)[0], ("--create-only", "--disk", "/dev/sda", "--part", "3", "--loader", ARM, "--label", "MIND Core"))
        self.assertEqual(firmware.mounted, ["/dev/sda4", "/dev/sda3"])
        self.assertFalse(any("--bootorder" in c or "-o" in c for c in firmware.calls))
        # Next time the entry is found, not made again.
        firmware.calls.clear()
        self.assertEqual(boot_switch.once(firmware, out=lambda *_: None), number)
        self.assertEqual(self.written(firmware), [("--bootnext", number)])

    def test_the_x86_loaders_entry_is_not_the_aarch64_one(self):
        firmware = Firmware(tree=[ubuntu_disk()], machine="aarch64", images={"/dev/sda": UBUNTU_GPT}, made=GPT_HD)
        firmware.entries["0007"] = {"active": True, "label": "MIND Core", "path": GPT_HD + rf"/File({X86})"}
        self.assertEqual(boot_switch.once(firmware, out=lambda *_: None), "0008")

    def test_volumes_found_by_label_at_any_number(self):
        disk = {"path": "/dev/sdc", "type": "disk", "model": "Flash Disk", "mountpoints": [None],
                "children": [{"path": "/dev/sdc1", "type": "part", "label": "MIND LOG", "mountpoints": [None]},
                             {"path": "/dev/sdc2", "type": "part", "label": "MIND CORE", "mountpoints": [None]}]}
        firmware = Firmware(tree=[system_disk(), disk], images={"/dev/sdc": mbr(number=2)},
                            made=f"HD(2,MBR,0x{SIGNATURE:x},0x{START:x},0x{SIZE:x})")
        boot_switch.once(firmware, out=lambda *_: None)
        self.assertEqual(self.written(firmware)[0][:5], ("--create-only", "--disk", "/dev/sdc", "--part", "2"))
        self.assertEqual(firmware.mounted, ["/dev/sdc1", "/dev/sdc2"])

    def test_refusals_on_ubuntus_disk_change_nothing(self):
        two = ubuntu_disk()
        two["children"].append({"path": "/dev/sda5", "type": "part", "label": "MIND LOG", "mountpoints": [None]})
        cases = {"MIND CORE mounted": Firmware(tree=[ubuntu_disk("/media/MIND CORE")], machine="aarch64", images={"/dev/sda": UBUNTU_GPT}),
                 "two MIND LOG volumes": Firmware(tree=[two], machine="aarch64", images={"/dev/sda": UBUNTU_GPT}),
                 "a machine with no loader": Firmware(tree=[ubuntu_disk()], machine="riscv64", images={"/dev/sda": UBUNTU_GPT})}
        for name, firmware in cases.items():
            with self.subTest(name), self.assertRaises(Refused):
                boot_switch.once(firmware, reboot=True, out=lambda *_: None)
            self.assertEqual(self.written(firmware), [], name)
            self.assertFalse(firmware.rebooted or firmware.next, name)

    def test_status_changes_nothing(self):
        firmware = Firmware()
        said = []
        boot_switch.status(firmware, out=said.append)
        self.assertEqual(self.written(firmware), [])
        self.assertIn("  Boot0001* ubuntu (Linux, booted now)", said)
        self.assertTrue(any(s.startswith("MIND Core disk: /dev/sdb") for s in said))


if __name__ == "__main__":
    unittest.main()
