"""Slots A and B in QEMU (351-UPD-0006; MC-9.1, 9.3), shared by tests/qemu_smoke.py (x86) and tests/aarch64_smoke.py.

On a raw disk image with both slots and the bootloader of the build:
- slot B staged for a trial boots on trial and the try is counted on the disk; once confirmed it boots as such;
- staged again and not confirmed, it gives way to slot A at the next boot;
- a trial slot with a damaged service is not loaded: A boots and B has no tries left;
- a newer record torn by a cut write is ignored for the older one;
- the file system is consistent after the bootloader's writes.

`boot(image, until)` boots the image and returns the console output up to `until`. Init's confirmation is 351-KRN-0014:
the host writes the confirmed record here, as the updater will.
"""
import shutil
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
import boot_slots  # noqa: E402

STARTED = "MIND CORE KERNEL: INIT STARTED"


def fsck(image):
    """fsck.fat -n of the image's partition: no error and no dirty bit."""
    volume = image.path.with_name("volume.img")
    with image.path.open("rb") as f:
        mbr = f.read(512)
        start, sectors = int.from_bytes(mbr[454:458], "little"), int.from_bytes(mbr[458:462], "little")
        f.seek(start * 512)
        volume.write_bytes(f.read(sectors * 512))
    check = subprocess.run(["fsck.fat", "-n", str(volume)], capture_output=True, text=True)
    volume.unlink()
    assert check.returncode == 0 and "Dirty bit" not in check.stdout + check.stderr, check.stdout + check.stderr


def run(boot, temp, volume, label):
    temp = Path(temp)
    image = boot_slots.Image.create(temp / "slots.img", boot_slots.layout(volume, temp / "slots", both=True))
    shutil.rmtree(temp / "slots")

    def booted(slot, *lines, trial=False):
        out = boot(image.path, STARTED).replace("\r", "")
        for line in (f"BOOT: SLOT {slot} LOADED" + (" ON TRIAL" if trial else "\n"), *lines):
            assert line in out, (line, out[-3000:])
        assert STARTED in out and "BOOT ERROR" not in out, out[-3000:]
        return out

    def newer():
        return boot_slots.newest(boot_slots.records(image.path))

    # A trial of B with one try: it runs on trial, its try counted on the other record first; confirmed, it stays.
    staged = boot_slots.write_next(image, slot="B", fallback="A", tries=1)
    booted("B", f"SEQUENCE {staged['sequence']}: SLOT B, NOT CONFIRMED, 1 TRIES LEFT", trial=True)
    assert newer()[1] == {"sequence": staged["sequence"] + 1, "slot": "B", "fallback": "A", "tries": 0, "confirmed": False}, newer()
    boot_slots.write_next(image, slot="B", fallback="A", confirmed=True)
    booted("B", "SLOT B, CONFIRMED")
    print(f"PASS ({label}): slot B boots on trial, its try counted on the disk first, and as confirmed once confirmed", flush=True)

    # Staged again and never confirmed: the next boot falls back to A.
    boot_slots.write_next(image, slot="B", fallback="A", tries=1)
    booted("B", trial=True)
    booted("A", "BOOT: SLOT B NOT CONFIRMED, NO TRIES LEFT")
    print(f"PASS ({label}): an unconfirmed trial of slot B falls back to slot A at the next boot", flush=True)

    # A trial slot with a damaged service: not loaded, A boots, and B is left with no tries.
    rtc = image.read("MIND/B/rtc.elf")
    image.write("MIND/B/rtc.elf", rtc[:-1] + bytes([rtc[-1] ^ 1]))
    staged = boot_slots.write_next(image, slot="B", fallback="A", tries=3)
    booted("A", "BOOT: SLOT B: rtc.elf: not as the manifest says")
    assert newer()[1] == {"sequence": staged["sequence"] + 2, "slot": "B", "fallback": "A", "tries": 0, "confirmed": False}, newer()
    image.write("MIND/B/rtc.elf", rtc)
    print(f"PASS ({label}): slot B with a damaged service is not loaded; slot A boots and B has no tries left", flush=True)

    # A newer record torn by a cut write (its first half written): ignored, the older record (B confirmed) counts.
    confirmed = boot_slots.write_next(image, slot="B", fallback="A", confirmed=True)
    k, _ = newer()
    other = boot_slots.FILES[1 - k]
    image.write(other, boot_slots.record(confirmed["sequence"] + 1, "A", confirmed=True)[:256] + image.read(other)[256:])
    booted("B", f"BOOT: RECORD {other.replace('/', chr(92))} DAMAGED, IGNORED")
    print(f"PASS ({label}): a boot record torn by a cut write is ignored for the other one", flush=True)
    fsck(image)
    print(f"PASS ({label}): the boot volume's file system is consistent after the bootloader's writes to the records", flush=True)
