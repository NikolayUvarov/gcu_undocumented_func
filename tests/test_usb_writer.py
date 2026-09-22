"""No block devices are opened: safety policy and I/O are tested on fake disks."""
import copy
import hashlib
import io
from pathlib import Path
import struct
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from scripts.write_usb_linux import copy_and_verify, image_info, validate_target


def disk():
    return {"path": "/dev/sdz", "type": "disk", "size": 2**30, "tran": "usb",
            "ro": False, "log-sec": 512, "maj:min": "65:0", "mountpoints": [None],
            "children": [{"path": "/dev/sdz1", "type": "part", "maj:min": "65:1",
                          "mountpoints": ["/media/test"]}]}


class WriterTests(unittest.TestCase):
    def test_allows_only_large_writable_usb_whole_disk(self):
        valid = disk()
        self.assertEqual(validate_target([valid], "/dev/sdz", 512, set()), valid)
        for field, value in [("tran", "sata"), ("tran", None), ("ro", True),
                             ("log-sec", 4096), ("size", 1), ("type", "part")]:
            wrong = copy.deepcopy(valid)
            wrong[field] = value
            with self.subTest(field=field, value=value), self.assertRaises(ValueError):
                validate_target([wrong], "/dev/sdz", 512, set())
        for path in ["/dev/sdz1", "/dev/missing"]:
            with self.assertRaises(ValueError):
                validate_target([valid], path, 512, set())

    def test_refuses_system_source_swap_and_active_storage_stack(self):
        for protected in [{"65:0"}, {"65:1"}]:
            with self.assertRaises(ValueError):
                validate_target([disk()], "/dev/sdz", 512, protected)
        for change in [{"mountpoints": ["[SWAP]"]}, {"mountpoints": ["/"]},
                       {"mountpoints": ["/boot/efi"]}, {"type": "crypt"}, {"type": "lvm"}, {"type": "raid1"}]:
            wrong = disk()
            wrong["children"][0].update(change)
            with self.assertRaises(ValueError):
                validate_target([wrong], "/dev/sdz", 512, set())

    def test_image_validation_before_device_write(self):
        data = bytearray(1024)
        data[450] = 0xef
        data[510:512] = data[1022:1024] = b"\x55\xaa"
        struct.pack_into("<II", data, 454, 1, 1)
        data[555:566] = b"MIND CORE  "
        data[566:574] = b"FAT16   "
        self.assertEqual(image_info(io.BytesIO(data)), (1024, hashlib.sha256(data).hexdigest()))
        for offset in [450, 454, 510, 555, 566, 1022]:
            bad = bytearray(data)
            bad[offset] ^= 0xff
            with self.assertRaises(ValueError):
                image_info(io.BytesIO(bad))

    def test_multi_chunk_write_readback_and_stale_gpt_cleanup(self):
        data = bytes(range(256)) * (5 * 4096)
        capacity = len(data) + 65536
        target = io.BytesIO(b"\xa5" * capacity)
        progress = []
        copy_and_verify(io.BytesIO(data), target, len(data), capacity,
                        hashlib.sha256(data).hexdigest(), lambda: None, progress.append)
        result = target.getvalue()
        self.assertEqual(result[:len(data)], data)
        self.assertEqual(result[-33 * 512:], bytes(33 * 512))
        self.assertEqual(result[len(data):-33 * 512], b"\xa5" * (65536 - 33 * 512))
        self.assertEqual(progress[-1], 100)

    def test_full_sized_image_is_never_damaged_by_tail_clear(self):
        data = bytes(range(256)) * 4
        target = io.BytesIO(bytes(len(data)))
        copy_and_verify(io.BytesIO(data), target, len(data), len(data),
                        hashlib.sha256(data).hexdigest(), lambda: None)
        self.assertEqual(target.getvalue(), data)

    def test_corrupt_readback_short_image_and_invalid_sizes_fail(self):
        data = bytes(1024)
        target = io.BytesIO(bytes(2048))
        def corrupt():
            target.seek(0)
            target.write(b"!")
        with self.assertRaises(OSError):
            copy_and_verify(io.BytesIO(data), target, 1024, 2048,
                            hashlib.sha256(data).hexdigest(), corrupt)
        with self.assertRaises(OSError):
            copy_and_verify(io.BytesIO(b"short"), target, 1024, 2048, "unused", lambda: None)
        for size, capacity in [(0, 2048), (513, 2048), (1024, 512), (1024, 2049)]:
            with self.assertRaises(ValueError):
                copy_and_verify(io.BytesIO(data), target, size, capacity, "unused", lambda: None)


if __name__ == "__main__":
    unittest.main()
