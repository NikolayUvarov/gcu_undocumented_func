"""Slots A and B in QEMU (351-UPD-0006; MC-9.1, 9.3), shared by tests/qemu_smoke.py (x86) and tests/aarch64_smoke.py.

On a raw disk image with both slots and the bootloader of the build:
- slot B staged for a trial boots on trial and the try is counted on the disk; once confirmed it boots as such;
- staged again and not confirmed, it gives way to slot A at the next boot;
- a trial slot with a damaged service is not loaded: A boots and B has no tries left;
- a newer record torn by a cut write is ignored for the older one;
- the file system is consistent after the bootloader's writes;
- vfs_server's update zone is the slot that did not boot (351-UPD-0008);
- with the updater's test stand-in on the volume, init's client of vfs_server badged for the update zone fills the slot
  that did not boot and writes a boot record whole in place, and nothing else (351-KRN-0022);
- `refused_trial`: slot B on trial with a kernel that refuses the loader's handoff restarts the machine, and slot A
  boots (351-KRN-0167);
- `own_loaders` (351-UPD-0010): slot B holding a loader other than the one at EFI/BOOT/ boots through it, on trial and
  once confirmed; one reset while it runs, one that cannot load its slot and one that is not as the manifest lists it
  leave the machine on slot A; a slot without a loader of its own boots as before. In `run` both slots hold the
  build's loader, the same file, and none is started.

`boot(image, until)` boots the image and returns the console output up to `until`; the volume has no keystore
(`quiet_volume`). Init's confirmation is 351-KRN-0014: the host writes the confirmed record here, as the updater will.
"""
import shutil
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
import boot_slots  # noqa: E402
import sign_manifest  # noqa: E402

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


def quiet_volume(volume, out, updater=None):
    """A copy of the volume without keystore, which stores a new device key at the first boot: a boot stopped in that
    write would leave the file system torn, and the checks stop boots early. With `updater`, the stand-in as
    `updater.elf`; without, no updater: it confirms a trial on the disk as soon as init has (351-UPD-0021), and here
    the host writes the records. Signed again."""
    shutil.copytree(volume, out)
    (out / "keystore.elf").unlink(missing_ok=True)
    if updater:
        shutil.copyfile(updater, out / "updater.elf")
    else:
        (out / "updater.elf").unlink(missing_ok=True)
    sign_manifest.sign_volume(out)
    return out


def run(boot, temp, volume, label, updater=None):
    temp = Path(temp)
    source = quiet_volume(volume, temp / "volume")
    image = boot_slots.Image.create(temp / "slots.img", boot_slots.layout(source, temp / "slots", both=True))
    shutil.rmtree(source)
    shutil.rmtree(temp / "slots")

    def booted(slot, *lines, trial=False, until=STARTED):
        out = boot(image.path, until).replace("\r", "")
        for line in (f"BOOT: SLOT {slot} LOADED" + (" ON TRIAL" if trial else "\n"), *lines):
            assert line in out, (line, out[-3000:])
        # The slot's own loader is the same file as the one at EFI/BOOT/: not started (351-UPD-0010).
        assert STARTED in out and "BOOT ERROR" not in out and "OWN LOADER" not in out, out[-3000:]
        return out

    def newer():
        return boot_slots.newest(boot_slots.records(image.path))

    # A trial of B with one try: it runs on trial, its try counted on the other record first; confirmed, it stays.
    staged = boot_slots.write_next(image, slot="B", fallback="A", tries=1)
    booted("B", f"SEQUENCE {staged['sequence']}: SLOT B, NOT CONFIRMED, 1 TRIES LEFT", trial=True)
    assert newer()[1] == {"sequence": staged["sequence"] + 1, "slot": "B", "fallback": "A", "tries": 0, "confirmed": False}, newer()
    boot_slots.write_next(image, slot="B", fallback="A", confirmed=True)
    zone = "[VFS] UPDATE ZONE: MIND/A AND THE BOOT RECORDS, FOR THE UPDATER'S BADGE"
    booted("B", "SLOT B, CONFIRMED", zone, until=zone)
    print(f"PASS ({label}): slot B boots on trial, its try counted on the disk first, and as confirmed once confirmed; "
          "vfs_server's update zone is then slot A", flush=True)

    # Staged again and never confirmed: the next boot falls back to A.
    boot_slots.write_next(image, slot="B", fallback="A", tries=1)
    booted("B", trial=True)
    zone = "[VFS] UPDATE ZONE: MIND/B AND THE BOOT RECORDS, FOR THE UPDATER'S BADGE"
    booted("A", "BOOT: SLOT B NOT CONFIRMED, NO TRIES LEFT", zone, until=zone)
    print(f"PASS ({label}): an unconfirmed trial of slot B falls back to slot A at the next boot, and the update zone is "
          "slot B again", flush=True)

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
    if updater:
        update_zone(boot, temp, volume, label, updater)
    else:
        print(f"SKIP ({label}): the update zone's client: no test stand-in for the updater was built", flush=True)


def refused_trial(boot, temp, volume, kernel, label):
    """Slot B on trial holds `kernel`, built with `loader-abi-test`: it refuses the handoff, says why and restarts the
    machine; the loader then boots slot A, B's one try spent. `boot` lets the machine restart."""
    temp = Path(temp)
    source = quiet_volume(volume, temp / "volume")
    other = temp / "other"
    shutil.copytree(source, other)
    (other / "kernel.elf").write_bytes(Path(kernel).read_bytes())
    sign_manifest.sign_volume(other)
    slots = boot_slots.layout(source, temp / "slots", both=True)
    for name in boot_slots.boot_set(other):
        shutil.copyfile(other / name, slots / "MIND" / "B" / name)
    image = boot_slots.Image.create(temp / "refused.img", slots)
    for path in (source, other, slots):
        shutil.rmtree(path)
    boot_slots.write_next(image, slot="B", fallback="A", tries=1)
    out = boot(image.path, STARTED).replace("\r", "")
    at = 0
    for line in ("BOOT: SLOT B LOADED ON TRIAL", "KERNEL STOPPED: THE BOOTLOADER HANDS OVER VERSION", "TRIAL: RESTARTING INTO THE FALLBACK",
                 "BOOT: SLOT B NOT CONFIRMED, NO TRIES LEFT", "BOOT: SLOT A LOADED\n", STARTED):
        at = out.find(line, at)
        assert at >= 0, (line, out[-3000:])
    print(f"PASS ({label}): a trial whose kernel refuses the loader's handoff restarts the machine, and the loader boots "
          "slot A with B's try spent", flush=True)


def update_zone(boot, temp, volume, label, updater):
    """The volume with the stand-in as `updater.elf`, signed again and laid out in slots; slot A boots, so B is the
    zone. The stand-in tries each case at start and logs the outcomes in one line."""
    source = quiet_volume(volume, temp / "zone-volume", updater)
    image = boot_slots.Image.create(temp / "zone.img", boot_slots.layout(source, temp / "zone-slots", both=True))
    shutil.rmtree(source)
    shutil.rmtree(temp / "zone-slots")
    record = image.read(boot_slots.FILES[0])
    out = boot(image.path, "[UPDATER-STUB] ZONE").replace("\r", "")
    line = ("[UPDATER-STUB] ZONE MIND/B: STUB.TXT WRITTEN; MIND/BOOT0 WHOLE WRITTEN, A PART INVALID, MADE AGAIN DENIED; "
            "MIND/A DENIED; EFI DENIED; MIND DENIED")
    assert line in out, out[-3000:]
    assert image.read("MIND/B/STUB.TXT") == b"STAGED BY THE UPDATER'S STAND-IN\n"
    assert image.read(boot_slots.FILES[0]) == record
    fsck(image)
    print(f"PASS ({label}): the updater's client of vfs_server fills the slot that did not boot and writes a boot record "
          "whole in place; the running slot, EFI, a new file in MIND, part of a record and a record made afresh are "
          "refused", flush=True)


def own_loaders(boot, temp, volume, label):
    """351-UPD-0010: slot B holds the build's loader with bytes past its sections, another file that loads the same,
    listed in B's manifest; slot A and EFI/BOOT/ hold the build's. data/pause.txt is there, 0 but for the reset."""
    temp = Path(temp)
    source = quiet_volume(volume, temp / "volume")
    (source / "data").mkdir(exist_ok=True)
    (source / "data/pause.txt").write_text("0\n")
    sign_manifest.sign_volume(source)
    [(path, name)] = boot_slots.loaders(source)
    other = temp / "other"
    shutil.copytree(source, other)
    with open(other / path, "ab") as loader:
        loader.write(b"slot B's own loader, another file\0")
    sign_manifest.sign_volume(other)
    slots = boot_slots.layout(source, temp / "slots", both=True)
    for file in boot_slots.boot_set(other):
        shutil.copyfile(other / file, slots / "MIND/B" / file)
    shutil.copyfile(other / path, slots / "MIND/B" / name)
    image = boot_slots.Image.create(temp / "loaders.img", slots)
    for tree in (source, other, slots):
        shutil.rmtree(tree)
    started = f"BOOT: SLOT B'S OWN LOADER, STARTED BY THE LOADER AT EFI/BOOT/"

    def booted(*lines, until=STARTED):
        out = boot(image.path, until).replace("\r", "")
        at = 0
        for line in lines:
            at = out.find(line, at)
            assert at >= 0, (line, out[-3000:])
        assert "BOOT ERROR" not in out and "BOOT PANIC" not in out, out[-3000:]
        return out

    def newer():
        return boot_slots.newest(boot_slots.records(image.path))[1]

    # On trial: its try counted once, by the loader at EFI/BOOT/, before the slot's own loader starts; confirmed, the same.
    staged = boot_slots.write_next(image, slot="B", fallback="A", tries=1)
    booted(f"SEQUENCE {staged['sequence']}: SLOT B, NOT CONFIRMED, 1 TRIES LEFT", "BOOT: SLOT B: STARTING ITS OWN LOADER", started + ", ON TRIAL",
           "BOOT: SLOT B LOADED ON TRIAL", STARTED)
    assert newer() == {"sequence": staged["sequence"] + 1, "slot": "B", "fallback": "A", "tries": 0, "confirmed": False}, newer()
    boot_slots.write_next(image, slot="B", fallback="A", confirmed=True)
    booted("SLOT B, CONFIRMED", "BOOT: SLOT B: STARTING ITS OWN LOADER", started + "\n", "BOOT: SLOT B LOADED\n", STARTED)
    print(f"PASS ({label}): slot B holding a loader of its own boots through it on trial, its try counted once before, "
          "and through it again once confirmed", flush=True)

    # Reset while the slot's own loader waits (its pause for a photo): its try was spent, the next boot is slot A's.
    image.write("data/pause.txt", b"2\n")
    boot_slots.write_next(image, slot="B", fallback="A", tries=1)
    booted("BOOT: SLOT B: STARTING ITS OWN LOADER", started + ", ON TRIAL", "S FOR A PHOTO OF THESE LINES", until="S FOR A PHOTO OF THESE LINES")
    image.write("data/pause.txt", b"0\n")
    out = booted("BOOT: SLOT B NOT CONFIRMED, NO TRIES LEFT", "BOOT: SLOT A LOADED\n", STARTED)
    assert "OWN LOADER" not in out, out[-3000:]
    print(f"PASS ({label}): a reset while slot B's own loader runs leaves its trial spent; the next boot is slot A's", flush=True)

    # Its own loader cannot load its slot (a service damaged): it returns, and slot A boots at once, B with no tries.
    rtc = image.read("MIND/B/rtc.elf")
    image.write("MIND/B/rtc.elf", rtc[:-1] + bytes([rtc[-1] ^ 1]))
    staged = boot_slots.write_next(image, slot="B", fallback="A", tries=3)
    booted(started + ", ON TRIAL", "BOOT: SLOT B: rtc.elf: not as the manifest says", "; BACK TO THE LOADER AT EFI/BOOT/",
           f"BOOT: SLOT B: {path}: it returned: it could not load its slot", "BOOT: SLOT A LOADED\n", STARTED)
    assert newer() == {"sequence": staged["sequence"] + 2, "slot": "B", "fallback": "A", "tries": 0, "confirmed": False}, newer()
    image.write("MIND/B/rtc.elf", rtc)
    print(f"PASS ({label}): slot B's own loader that cannot load its slot returns; slot A boots at once and B has no "
          "tries left", flush=True)

    # Not as B's manifest lists it: never started; slot A boots, B with no tries.
    loader = image.read(f"MIND/B/{name}")
    image.write(f"MIND/B/{name}", loader[:-1] + bytes([loader[-1] ^ 1]))
    staged = boot_slots.write_next(image, slot="B", fallback="A", tries=3)
    out = booted(f"BOOT: SLOT B: {path}: not as the manifest says", "BOOT: SLOT A LOADED\n", STARTED)
    assert "OWN LOADER" not in out, out[-3000:]
    assert newer() == {"sequence": staged["sequence"] + 2, "slot": "B", "fallback": "A", "tries": 0, "confirmed": False}, newer()
    print(f"PASS ({label}): a slot's loader that is not as its manifest lists it is never started; slot A boots", flush=True)

    # A slot without a loader of its own, as one staged before: the loader at EFI/BOOT/ loads it, as before.
    image.remove(f"MIND/B/{name}")
    boot_slots.write_next(image, slot="B", fallback="A", tries=1)
    out = booted("BOOT: SLOT B LOADED ON TRIAL", STARTED)
    assert "OWN LOADER" not in out, out[-3000:]
    fsck(image)
    print(f"PASS ({label}): a slot without a loader of its own boots through the loader at EFI/BOOT/, as before; the "
          "file system is consistent", flush=True)
