#!/usr/bin/env python3
"""Verify the packaged files, then boot the actual RAW image as a USB device (x86_64, or aarch64 with --arch aarch64)."""
import argparse
import datetime
import hashlib
import os
from pathlib import Path
import re
import shutil
import struct
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from scripts.make_usb_image import ARCHES, LOG_SECTORS, ROOT, check_image, find_qemu_img, pack, qemu_path, read_payloads, slots, store_superblock
import qemu_smoke
sys.path.insert(0, str(ROOT / "scripts"))
import release as release_tool  # noqa: E402
import sign_manifest  # noqa: E402
from qemu_smoke import MTOOLS_ENV, VM, canon, files_check, fsck_volume, heap_used, require, screen_text, status_line, task_rows


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--arch", choices=sorted(ARCHES), default="x86_64")
    parser.add_argument("--image", type=Path, help="default: dist/mind-core-usb.img (dist/mind-core-usb-aarch64.img)")
    parser.add_argument("--qemu", default=os.environ.get("QEMU"))
    parser.add_argument("--firmware", default="OVMF.fd")
    parser.add_argument("--aavmf-code", default="/usr/share/AAVMF/AAVMF_CODE.fd")
    parser.add_argument("--aavmf-vars", default="/usr/share/AAVMF/AAVMF_VARS.fd")
    parser.add_argument("--cpus", type=int, default=4)
    args = parser.parse_args()
    args.image = args.image or ROOT / ARCHES[args.arch]["image"]
    args.qemu = args.qemu or f"qemu-system-{args.arch}"
    check_image(args.image, read_payloads(ROOT / ARCHES[args.arch]["root"], args.arch))
    qemu_smoke.IMAGE, qemu_smoke.BOOT_EFI = ARCHES[args.arch]["root"], ARCHES[args.arch]["efi"]  # what files_check compares
    with args.image.open("rb") as image:
        image.seek(450)
        assert image.read(1) == b"\xef", "USB image must mark the UEFI system partition"
    # A copy boots without a snapshot, so that what the system writes on its log partition can be read back.
    work = Path(tempfile.mkdtemp(prefix=".mind-usb-image-", dir=args.image.parent))
    booted = work / args.image.name
    shutil.copyfile(args.image, booted)
    try:
        run(args, booted)
        store_partition_check(args, work)
        slots_check(args, work)
    finally:
        shutil.rmtree(work, ignore_errors=True)


def partitions(image):
    """The MBR's entries, (number, type, first sector, count), and the disk's signature."""
    with Path(image).open("rb") as f:
        mbr = f.read(512)
    entries = [(i + 1, mbr[446 + i * 16 + 4], *struct.unpack_from("<II", mbr, 446 + i * 16 + 8)) for i in range(4) if mbr[446 + i * 16 + 4]]
    return entries, struct.unpack_from("<I", mbr, 440)[0]


def sector(image, lba):
    with Path(image).open("rb") as f:
        f.seek(lba * 512)
        return f.read(512)


def store_lines(vm):
    """The block store's READY line, once it is up, and init's lines: through dmesg, which `logs` does not drain."""
    for _ in range(60):
        ready = vm.command("dmesg -s blockstore", raw=True)
        if "[BLOCKSTORE] READY" in ready:
            return ready, vm.command("dmesg -s init", raw=True)
        time.sleep(1)
    raise AssertionError(f"the block store never got ready: {ready[-1500:]}")


def store_partition_check(args, work):
    """307-KRN-0177, on fresh copies of the stick: a blank NVMe disk beside it is not given to the store, which takes the
    stick's partition; a second disk with a partition of the store's type and superblock makes two candidates, and init
    takes neither and names both, so the store runs on ramdisk#1 and neither is written; the stick's partition made
    blank is not taken, until store.txt names it: then the store takes it blank and formats it."""
    entries, signature = partitions(args.image)
    _, kind, first, count = entries[2]
    assert kind == 0x7F, entries
    taken = f"[INIT] THE BLOCK STORE'S PARTITION: usb_storage PARTITION 3 ({{}}), {count} SECTORS FROM {first}, WINDOW 1"

    def boot(name, extra=None, change=None):
        stick = work / f"stick-{name}.img"
        shutil.copyfile(args.image, stick)
        if change:
            change(stick)
        nvme = ("-drive", f"format=raw,file={qemu_path(extra, args.qemu)},if=none,id=storecheck", "-device", "nvme,serial=storecheck,drive=storecheck") if extra else ()
        vm = VM(args, qemu_path(stick, args.qemu), usb=True, snapshot=False, extra=nvme)
        try:
            return (stick, *store_lines(vm))
        finally:
            vm.close()

    # A blank NVMe disk: no table, so nothing of it is the store's; the stick's partition is taken, and the disk stays blank.
    blank = work / "nvme-blank.img"
    with blank.open("wb") as f:
        f.truncate(16 << 20)
    stick, ready, init = boot("blank-nvme", blank)
    require(init, "[INIT] nvme: NO PARTITION TABLE, 0 PARTITIONS")
    require(init, taken.format("ITS TYPE AND SUPERBLOCK"))
    require(ready, f"SECTORS=1/{count} ")
    assert not any(blank.read_bytes()), "the blank NVMe disk was written"
    # Two candidates: neither taken, both named; the store on ramdisk#1 (8 MiB), and neither partition written.
    other = work / "nvme-candidate.img"
    with other.open("wb") as f:
        f.truncate(16 << 20)
        mbr = bytearray(512)
        mbr[446 + 4] = 0x7F
        struct.pack_into("<II", mbr, 446 + 8, 2048, 30720)
        struct.pack_into("<I", mbr, 440, 0x5EC0DD15)
        mbr[510:512] = b"\x55\xaa"
        f.write(mbr)
        f.seek(2048 * 512)
        f.write(store_superblock())
    before = hashlib.sha256(other.read_bytes()).hexdigest()
    stick, ready, init = boot("two", other)
    require(init, "[INIT] THE STORE'S PARTITION REFUSED: 2 CANDIDATES, NONE TAKEN")
    require(init, "[INIT] A CANDIDATE: nvme PARTITION 1 (ITS TYPE AND SUPERBLOCK)")
    require(init, "[INIT] A CANDIDATE: usb_storage PARTITION 3 (ITS TYPE AND SUPERBLOCK)")
    require(ready, "SECTORS=1/16384 ")
    assert hashlib.sha256(other.read_bytes()).hexdigest() == before, "the NVMe disk's candidate was written"
    assert sector(stick, first) == store_superblock(), "the stick's candidate was written"
    # The stick's partition made blank: the store's type alone is not enough...
    stick, ready, init = boot("blank", change=lambda image: _blank(image, first))
    require(init, "[INIT] usb_storage PARTITION 3: THE STORE'S TYPE WITHOUT ITS SUPERBLOCK: NOT TAKEN, UNLESS store.txt NAMES IT")
    require(init, "[INIT] NO PARTITION OF THE STORE'S")
    require(ready, "SECTORS=1/16384 ")
    assert not any(sector(stick, first)), "the blank partition was written"
    # ...until store.txt at the boot volume's root names it by its PARTUUID: the store takes it blank and formats it.
    def named(image):
        _blank(image, first)
        word = work / "store.txt"
        word.write_text(f"# the block store's own partition\npartition {signature:08x}-03\n")
        subprocess.run(["mcopy", "-i", f"{image}@@{entries[0][2] * 512}", str(word), "::store.txt"], check=True, env=MTOOLS_ENV)
    stick, ready, init = boot("named", change=named)
    require(init, taken.format("NAMED IN STORE.TXT"))
    require(ready, f"SECTORS=1/{count} ")
    assert sector(stick, first).startswith(b"MIND-STO"), "the store did not format the partition store.txt names"
    print(f"PASS ({args.arch}): the store's own partition (307-KRN-0177): a blank NVMe disk not given to it; two candidates refused, "
          "both named, neither written; a blank partition of its type not taken, then taken and formatted once store.txt names it", flush=True)


def _blank(image, lba):
    with Path(image).open("r+b") as f:
        f.seek(lba * 512)
        f.write(bytes(512))


def slots_check(args, work):
    """351-UPD-0016: the same build packed as `make_usb_image.py --slots --channel` packs it, with a channel of release 1
    naming its manifest, signed with the public test release key the updater is built with: it boots from slot A,
    vfs_server's update zone is slot B, and the updater knows the running version."""
    payloads = read_payloads(ROOT / ARCHES[args.arch]["root"], args.arch)
    expires = (datetime.datetime.now(datetime.timezone.utc) + datetime.timedelta(days=30)).strftime("%Y-%m-%dT%H:%M:%SZ")
    body = release_tool.channel_bytes("stable", 1, 1, expires, {args.arch: hashlib.sha256(payloads["MANIFEST"]).hexdigest()})
    channel = body + b"ed25519 " + sign_manifest.sign(release_tool.TEST_RELEASE_SEED, body).hex().encode() + b"\n"
    image = work / "slots.img"
    pack(slots(payloads, args.arch, channel), image, find_qemu_img(os.environ.get("QEMU_IMG")), args.arch)
    vm = VM(args, qemu_path(image, args.qemu), usb=True, snapshot=False)
    try:
        require(vm.log.replace("\r", ""), "BOOT: SLOT A LOADED\n")
        require(vm.service_logs("vfs_server", "UPDATE ZONE"), "[VFS] UPDATE ZONE: MIND/B AND THE BOOT RECORDS, FOR THE UPDATER'S BADGE")
        require(vm.service_logs("updater", "[UPDATER] SLOT A"), "[UPDATER] SLOT A, VERSION 1; THE NEWER RECORD: SLOT A CONFIRMED")
        # 351-UPD-0023: the applications are packed into slot A with the services, and the loader opens them there.
        require(vm.service_logs("loader", "[LOADER] APPLICATIONS FROM"), "[LOADER] APPLICATIONS FROM MIND/A/")
        # 109-KRN-0117: the updater read the channel through a parser of its own, started with its loader client, which
        # carries BADGE_HELPERS_ONLY (1); the parser ended with the check, so the shared parse service runs alone.
        real = vm.services()
        servers = {int(ep): int(server) for ep, server in re.findall(r"^EP=(\d+) .*SERVER=(\d+)", vm.command("endpoints", raw=True), re.M)}
        held = re.search(r"^SLOT=5 GEN=\d+ KIND=1 .*BADGE=1 EP=(\d+)", vm.command(f"stat caps {real['updater']}", raw=True), re.M)
        assert held and servers.get(int(held[1])) == real["loader"], held
        for _ in range(20):
            parsers = len(re.findall(r"^\d+ parse ", vm.command("ps", raw=True), re.M))
            if parsers == 1:
                break
            time.sleep(.25)
        assert parsers == 1, parsers
        # 351-APP-0088: files reads the kernel from the slot that booted, whole; the root has none.
        pid = int(re.search(r"PID=(\d+) NAME=files BACKGROUND", vm.command("run files &"))[1])
        output = ""
        for _ in range(50):
            output += vm.command(f"logs {pid}")
            if "[FILES] DONE" in output:
                break
            time.sleep(.2)
        # The path names the slot; the line's end may come in the next `logs`, which drains as the output arrives.
        size = len(payloads["kernel.elf"])
        require(output, f"READ MIND/A/kernel.elf {size}/{size} BYTES MAGIC=7F454C46")
        vm.command(f"kill {pid}")
        assert "PANIC" not in vm.log, vm.log[-2000:]
    finally:
        vm.close()
        log = Path(tempfile.gettempdir()) / f"mind-core-usb-slots{'' if args.arch == 'x86_64' else '-' + args.arch}.log"
        log.write_text(vm.log)
    print(f"PASS ({args.arch}): the image packed with slots boots from slot A; the update zone is slot B; the updater reads "
          "the running version from the release channel kept in slot A; its applications run from slot A; files reads slot A's kernel whole", flush=True)


def debug_mode(vm, name):
    # 211-KRN-0053: with log:debug.txt the loader gives programs a log client, so what uptime prints reaches this boot's
    # log file; without it, a program's lines stay on its console.
    require(vm.command("run uptime"), "up ")
    require(vm.command("write log:debug.txt on"), "WROTE")
    require(vm.command("run uptime"), "up ")
    require(vm.command("sync"), "OK")  # ordinary records wait up to 30 s; a flush saves them (211-KRN-0223)
    for _ in range(20):
        saved = vm.command(f"cat log:{name}", raw=True)
        if re.search(r"uptime\(\d+\) up \d", saved):
            break
        time.sleep(.5)
    assert re.search(r"uptime\(\d+\) up \d", saved), saved[-1500:]
    assert len(re.findall(r"uptime\(\d+\) up \d", saved)) == 1, "only the run in debug mode is logged"
    require(saved, "DEBUG MODE ON (log:debug.txt)")
    # A program that asks for the log keeps the shell's client, which reads: dmesg still shows the system log.
    require(vm.command("dmesg -s loader"), "[LOADER] DEBUG MODE ON")
    require(vm.command("rm log:debug.txt"), "OK")


def unplugged(vm, booted, name):
    # 211-KRN-0050: with the boot disk unplugged, a program on it is refused soon, with the reason, and the shell answers;
    # plugged in again, programs start again. memmap was not run before, so none of it is in vfs_server's cache.
    # Only the shell's built-in commands from here until the disk is back: dmesg is a program on the disk.
    vm.hmp("device_del usbstick")
    require(vm.service_logs("usb_host", "DISCONNECTED"), "DISCONNECTED")
    started = time.monotonic()
    require(vm.command("run memmap"), "CANNOT READ THE PROGRAM")  # the command's own wait is 8 s
    took = time.monotonic() - started
    require(vm.command("ps"), "shell")
    require(vm.service_logs("vfs_server", "DRIVE DOES NOT ANSWER"), "DRIVE DOES NOT ANSWER")
    print(f"unplugged: run memmap refused in {took:.1f} s")
    vm.hmp(f"drive_add 0 if=none,id=usbdisk2,format=raw,file={booted}")
    vm.hmp("device_add usb-storage,drive=usbdisk2,id=usbstick")
    for _ in range(40):  # vfs_server asks the drive again after its quiet time
        time.sleep(2)
        if "NAME=memmap" in vm.command("run memmap &"):
            break
    else:
        raise AssertionError("memmap does not start after the disk is plugged in again")
    require(vm.service_logs("vfs_server", "DRIVE ANSWERS AGAIN"), "DRIVE ANSWERS AGAIN")
    require(vm.command("sync"), "OK")
    # The records of the time without the disk reach this boot's log once it is back (the journal kept them).
    for _ in range(40):
        saved = vm.command(f"cat log:{name}", raw=True)
        if "DRIVE ANSWERS AGAIN" in saved:
            break
        time.sleep(1)
    require(saved, "DRIVE DOES NOT ANSWER")
    # The journal leaves the log volume marked dirty between its saves (211-KRN-0223): a flush marks it clean, as
    # before a stick is taken out; the image is read on the host as this leaves it.
    require(vm.command("sync"), "OK")
    # 000-KRN-0079: another medium in its place, as a stick rewritten with another build elsewhere and put back: the
    # same partition table and disk signature, another commit in the boot manifests. vfs_server, which saw the drive
    # not answer, finds it is not the stick it mounted: nothing it cached is written to it, its volumes fail. The new
    # stick gets another device ID: QEMU may not have let go of the old one yet.
    vm.hmp("device_del usbstick")
    require(vm.service_logs("usb_host", "DISCONNECTED"), "DISCONNECTED")
    require(vm.command("run memmap"), "CANNOT READ THE PROGRAM")
    require(vm.service_logs("vfs_server", "DRIVE DOES NOT ANSWER"), "DRIVE DOES NOT ANSWER")
    other = booted.with_name(booted.stem + "-other" + booted.suffix)
    shutil.copyfile(booted, other)
    data = bytearray(other.read_bytes())
    manifests = [m for m in re.finditer(rb"MIND-MANIFEST \d+\nkey [0-9a-f]{16}\ncommit ", data) if m.start() % 512 == 0]
    assert manifests, "no boot manifest found in the image"
    for m in manifests:
        data[m.end()] = ord("0") if data[m.end()] != ord("0") else ord("1")
    other.write_bytes(data)
    assert other.read_bytes()[:512] == booted.read_bytes()[:512], "the copy keeps the stick's sector 0"
    before = hashlib.sha256(other.read_bytes()).hexdigest()
    for command in (f"drive_add 0 if=none,id=usbdisk3,format=raw,file={other}", "device_add usb-storage,drive=usbdisk3,id=usbstick3"):
        answer = vm.hmp(command)
        assert answer.strip() in ("", "OK"), (command, answer)
    require(vm.service_logs("usb_host", "SUPER SPEED"), "SUPER SPEED")
    for _ in range(40):  # vfs_server asks the drive again after its quiet time
        time.sleep(2)
        vm.command("ls log:", raw=True)
        log = vm.service_logs("vfs_server")
        if "ANOTHER MEDIUM" in log:
            break
    require(log, "CAME BACK WITH ANOTHER MEDIUM")
    require(vm.command("run memmap"), "CANNOT READ THE PROGRAM")
    vm.command("sync")
    time.sleep(3)
    assert hashlib.sha256(other.read_bytes()).hexdigest() == before, "the old volumes wrote to another medium"
    other.unlink()


def run(args, booted):
    vm = VM(args, qemu_path(booted, args.qemu), usb=True, snapshot=False)
    try:
        # The baseline once the boot has settled (on aarch64 a service still frees memory of its start a moment later).
        baseline = heap_used(vm)
        for _ in range(10):
            time.sleep(1)
            if heap_used(vm) == baseline:
                break
            baseline = heap_used(vm)
        cpus = vm.command("cpus")
        assert cpus.count("ONLINE=true") == args.cpus, cpus
        listing = vm.command("list")
        for name in ["app", "app2", "clock", "dzen-clock"]:
            require(listing, name)
        for pid, name in enumerate(["app", "app2", "clock", "dzen-clock"], 1):
            require(vm.command(f"run {name} &"), f"PID={pid} NAME={name} BACKGROUND")
        first = task_rows(vm)
        time.sleep(1.2)  # a clock wakes when its second changes (000-APP-0012)
        second = task_rows(vm)
        assert set(second) == {1, 2, 3, 4}, second
        assert all(int(second[p][-1]) > int(first[p][-1]) for p in second), (first, second)
        require(vm.command("logs 2"), "PRIVATE HEAP SPRITE READY")
        require(vm.command("logs 3"), "[CLOCK]")
        require(vm.command("logs 4"), "[DZEN-CLOCK]")
        vm.send("fg 2\n")
        vm.expect("FOREGROUND PID=2")
        vm.send("\x1b\n")
        vm.expect("PID=2 EXITED. SHELL RESUMED.")
        time.sleep(.1); vm.collect(); vm.output = ""
        for pid in [1, 3, 4]:
            require(vm.command(f"kill {pid}"), f"KILLED PID={pid}")
        for _ in range(20):  # the kernel reclaims a killed task's memory as other suites wait for it
            if heap_used(vm) == baseline:
                break
            time.sleep(.1)
        assert heap_used(vm) == baseline, (heap_used(vm), baseline)
        # Files are read from the same USB drive: xHCI -> usb_host -> usb_storage -> vfs_server (issue 164).
        require(vm.service_logs("usb_host", "SUPER SPEED"), "(SUPER SPEED)")
        usb = vm.service_logs("usb_storage", "[USB] MASS STORAGE: ")
        require(usb, "[USB] MASS STORAGE: ")
        require(vm.service_logs("vfs_server", "[VFS] MOUNTED FAT16 FROM USB"), "[VFS] MOUNTED FAT16 FROM USB")
        # 307-KRN-0177: the store on its own partition, window 1 of the usb_storage vfs_server reads its volumes from. Read
        # with `logs`, a built-in (`logs` drains, so usb_storage's window line is in the output read above): the PIDs the
        # next checks expect stay free.
        entries, signature = partitions(booted)
        _, _, store_first, store_count = entries[2]
        ready = vm.service_logs("blockstore", "[BLOCKSTORE] READY")
        init = vm.service_logs("init", "[INIT] THE BLOCK STORE'S PARTITION")
        require(init, "[INIT] usb_storage: MBR, 3 PARTITIONS")
        require(init, f"[INIT] usb_storage PARTITION 3: TYPE 7F, {store_count} SECTORS FROM {store_first}, PARTUUID {signature:08x}-03")
        require(init, f"[INIT] THE BLOCK STORE'S PARTITION: usb_storage PARTITION 3 (ITS TYPE AND SUPERBLOCK), {store_count} SECTORS FROM {store_first}, WINDOW 1")
        require(usb + vm.service_logs("usb_storage"), f"[BLOCK] WINDOW 1 SET: {store_count} SECTORS FROM {store_first}")
        require(ready, f"SECTORS=1/{store_count} ")
        require(vm.command("run files &"), "PID=5 NAME=files BACKGROUND")
        files_check(vm, 5)
        vm.command("kill 5")
        assert "FAULT PID=" not in vm.command("faults")
        # The log partition (211-PRT-0006, 211-KRN-0019): mounted as log:, this boot's system log saved there.
        vfs = vm.command("dmesg -s vfs_server", raw=True)  # its buffered output was read above
        require(vfs, "[VFS] MOUNTED FAT16 AT LBA ")
        name = re.search(r"GOES TO LOG:(boot\d{4}\.log)", vfs)[1]
        require(vm.command("logger LOG-PARTITION-CHECK"), "LOGGED")
        require(vm.command("sync"), "OK")  # an ordinary record waits up to 30 s; a flush saves it (211-KRN-0223)
        for _ in range(20):
            saved = vm.command(f"cat log:{name}", raw=True)
            if "LOG-PARTITION-CHECK" in saved:
                break
            time.sleep(.5)
        require(saved, "THE SYSTEM LOG OF ONE BOOT")
        require(saved, "LOG-PARTITION-CHECK")
        require(vm.command("write log:note.txt written on MIND CORE"), "WROTE")
        log_tools_check(vm, name)
        require(vm.command("sync"), "OK")
        time.sleep(3)  # the journal's last save, flushed
        debug_mode(vm, name)
        unplugged(vm, booted, name)
    finally:
        vm.close()
        log = Path(tempfile.gettempdir()) / f"mind-core-usb-image{'' if args.arch == 'x86_64' else '-' + args.arch}.log"
        log.write_text(vm.log)
        print(f"QEMU log: {log}")
    # On the host, as any computer reads it: the boot log and the note on a clean FAT16 volume.
    with booted.open("rb") as image:
        mbr = image.read(512)
    assert mbr[466] == 0x0E, "the log partition"
    start = struct.unpack_from("<I", mbr, 470)[0]
    read = lambda file: subprocess.run(["mtype", "-i", f"{booted}@@{start * 512}", f"::{file}"], check=True, capture_output=True, env=MTOOLS_ENV).stdout
    require(read(name).decode(errors="replace"), "LOG-PARTITION-CHECK")
    assert read("note.txt") == b"written on MIND CORE\n", read("note.txt")
    # The hardware report (174-KRN-0038): this boot's beside its system log, and the firmware's ACPI tables.
    report = read(f"hw{name[4:8]}.txt").decode(errors="replace")
    for section in ("MIND CORE HARDWARE REPORT 1", "\nCPU\n", "Memory map (the firmware's)", "ACPI tables", "PCI functions", "The kernel's choices", "Kernel lines", "MIND CORE KERNEL: INIT STARTED"):
        assert section in report, (section, report[:2000])
    assert re.search(r"USB controller", report), report
    if args.arch == "x86_64":
        # 000-KRN-0081: the firmware's MTRRs and the PAT the kernel set, entry 1 write-combining for the framebuffer.
        assert re.search(r"\n  MTRR {14}\d+ variable ranges", report), report[:3000]
        require(report, "\n  PAT               0007040600070106\n")
        # 000-KRN-0220: each CPU's CR0 once it started, CD (bit 30) and NW (bit 29) clear: caching on.
        cr0 = re.search(r"\n  CR0 after start  ((?: [0-9A-F]{8})+)\n", report)
        assert cr0 and all(int(v, 16) & (3 << 29) == 0 for v in cr0[1].split()), report[:3000]
    # 550-KRN-0070: no function a driver was not granted masters the bus, bridges and display controllers aside.
    functions = re.findall(r"class ([0-9A-F]{6}) .*? command ([0-9A-F]{4}) status [0-9A-F]{4}(.*)", report)
    assert functions, report[:3000]
    masters = [(c, cmd, note) for c, cmd, note in functions if int(cmd, 16) & 4 and "granted" not in note and c[:2] not in ("03", "06")]
    assert not masters, masters
    for table in ("FACP", "DSDT", "APIC"):
        data = read(f"acpi/{table}.bin")
        assert data[:4] == table.encode() and len(data) == struct.unpack_from("<I", data, 4)[0], (table, len(data))
    assert read("acpi/RSDP.bin").startswith(b"RSD PTR ")
    fsck_volume(booted, start, LOG_SECTORS)
    # 307-KRN-0177: the store wrote its key into its own superblock, in place; the table and the boot volume are intact.
    entries, _ = partitions(booted)
    assert sector(booted, 0) == sector(args.image, 0), "the stick's table changed"
    superblock = sector(booted, entries[2][2])
    assert superblock.startswith(b"MIND-STO") and superblock != store_superblock(), "the store's key not in its superblock"
    fsck_volume(booted, entries[0][2], entries[0][3])
    print(f"PASS ({args.arch}): exact image contents; UEFI boot from USB RAW image; CPUs, all programs, private heap, fg/exit/kill/reclaim; "
          "VFS over xHCI USB mass storage through usb_host; the disk unplugged (a program refused with the reason, the shell answering) "
          "and plugged in again, another medium in its place left untouched; the boot's system log, the hardware report, the ACPI tables and a file on the log partition, read on the host; "
          "log: in help, df, fsck and fm; the block store on the stick's partition 3, window 1 of the same drive, its key in its superblock, the boot volume clean")


def log_tools_check(vm, name):
    """The tools on log: (211-APP-0013): help, df and fsck name it; fm lists it and shows the boot log in its viewer."""
    require(vm.command("help ls"), "log: the boot disk's log partition")
    assert re.search(r"log:\s.*FAT16", vm.command("df")), vm.command("df")
    require(vm.command("fsck log:"), "clean")
    def keys(data, text):
        vm.send_bytes(data)
        return status_line(vm, text)
    vm.send("fm\n")
    status_line(vm, "[FM] READY")
    keys(b"\x1b[12;3~", "DIALOG=VOLUME")  # Alt+F2: A:, ram:, log: (no model disk here)
    keys(b"\x1b[B", "DIALOG=VOLUME")
    keys(b"\x1b[B", "DIALOG=VOLUME")
    keys(b"\r", "RIGHT=/log: BRIEF")
    line = keys(b"\t", "ACTIVE=R")
    for _ in range(8):
        if f"CURRENT={name} " in line:
            break
        line = keys(b"\x1b[B", "ACTIVE=R")
    require(line, f"CURRENT={name} ")
    keys(b"\x1bOR", "VIEW=1")  # F3
    time.sleep(.3)
    screen = screen_text(vm)
    vm.serial()
    assert any(canon("THE SYSTEM LOG OF ONE BOOT") in row for row in screen), screen
    keys(b"\x1b", "VIEW=0")
    vm.send_bytes(b"\x1b[21~")  # F10
    require(vm.expect("EXITED. SHELL RESUMED."), "[FM] DONE")
    time.sleep(.1); vm.collect(); vm.output = ""


if __name__ == "__main__":
    main()
