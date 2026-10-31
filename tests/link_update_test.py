#!/usr/bin/env python3
"""Host test of scripts/link_update.py (351-KRN-0141): what goes over the cable, and how the updater's log is followed
across the trial restart (a scripted log, no machine)."""
import importlib.util, pathlib

ROOT = pathlib.Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location("link_update", ROOT / "scripts/link_update.py")
lu = importlib.util.module_from_spec(spec)
spec.loader.exec_module(lu)


class Scripted:
    """A link whose log reads are given in turn; None is a read while the machine restarts."""
    def __init__(self, reads):
        self.reads = list(reads)

    def log(self):
        return self.reads.pop(0) if len(self.reads) > 1 else self.reads[0]


def follow(reads, before=""):
    said = []
    try:
        return lu.follow(Scripted(reads), 5, said.append, pause=0, before=before), said
    except RuntimeError as e:
        return f"error: {e}", said


def test_release_files():
    manifest = (b"mind-manifest 1\nfile kernel.elf 10 aa 0 0\nfile shell.elf 20 bb 0 0\nfile memtest.elf 30 cc 0 0\n"
                b"file init.elf 40 dd 0 0\nfile LICENSES/LICENSE-MIT 5 ee 0 0\nfile voice/model.bin 6 ff 0 0\n"
                b"file EFI/BOOT/BOOTX64.EFI 7 ab 0 0\nfile app.elf 8 cd 0 0\nfile app2.elf 8 cd 0 0\n")
    files = lu.release_files(manifest, "x86_64", 7)
    # The boot files first, the loader (351-UPD-0010), then the applications (351-KRN-0151), a shared blob once; no
    # licence or model.
    assert files == ["blobs/aa", "blobs/bb", "blobs/dd", "blobs/ab", "blobs/cc", "blobs/cd", "releases/7/x86_64/MANIFEST",
                     "releases/7/x86_64/MANIFEST.SIG"], files
    # Another processor's loader is not this one's.
    assert "blobs/ab" not in lu.release_files(manifest, "aarch64", 7)


def test_follow():
    boot1 = "1 boot\n2 [UPDATER] CHECK: REFUSED: Missing\n"
    # Old refusals before the channel are not this update's; one between the read and the put (Missing) is not either.
    fetched = boot1 + "3 [UPDATER] CHECK: REFUSED: Missing\n4 [UPDATER] FETCH: VERSION 7 STAGED IN SLOT B\n5 [INIT] RESTARTING\n"
    boot2 = "1 boot\n2 [UPDATER] SLOT B ON TRIAL, VERSION 7\n3 [UPDATER] CHECK: REFUSED: Older\n4 [UPDATER] SLOT B CONFIRMED ON THE DISK: INIT CONFIRMED THE TRIAL BOOT\n"
    line, said = follow([fetched, None, None, boot2], before=boot1)
    assert line.startswith("4 [UPDATER] SLOT B CONFIRMED"), (line, said)
    assert "4 [UPDATER] FETCH: VERSION 7 STAGED IN SLOT B" in said and "2 [UPDATER] CHECK: REFUSED: Missing" not in said, said
    # A refusal of this update ends it.
    line, _ = follow([boot1 + "3 [UPDATER] CHECK: REFUSED: Signature\n"], before=boot1)
    assert line == "error: 3 [UPDATER] CHECK: REFUSED: Signature", line
    # Older before any trial: the machine runs a newer version.
    line, _ = follow([boot1 + "3 [UPDATER] CHECK: REFUSED: Older\n"], before=boot1)
    assert line == "error: 3 [UPDATER] CHECK: REFUSED: Older", line
    # Lines read once are not read again.
    line, said = follow([boot1 + "3 a\n", boot1 + "3 a\n4 b\n", boot1 + "3 a\n4 b\n5 [UPDATER] SLOT A CONFIRMED ON THE DISK\n"], before=boot1)
    assert line.startswith("5 ") and said.count("5 [UPDATER] SLOT A CONFIRMED ON THE DISK") == 1, said
    # No confirmation in time.
    line, _ = follow([boot1], before=boot1)
    assert line.startswith("error: no confirmation"), line
    # A read that lost its anchor (a long read of a large log, the MacBook Pro on 2026-10-31) holds old lines, an old
    # refusal among them: not taken for new; the next boot's log is (800-LNK-0034).
    late = "71482.148 [UPDATER] FETCH: 62 OF 104\n71483.511 [UPDATER] FETCH: 64 OF 104\n"
    stale = "68816.682 [UPDATER] CHECK stable: VERSION 6\n68816.683 [UPDATER] CHECK: REFUSED: Older\n"
    boot = "0.286 [INIT] LAUNCH\n2.860 [UPDATER] SLOT B ON TRIAL, VERSION 7\n3.019 [UPDATER] SLOT B CONFIRMED ON THE DISK\n"
    line, said = follow([late, stale, boot], before="71400.000 x\n")
    assert line.startswith("3.019 [UPDATER] SLOT B CONFIRMED"), (line, said)
    assert not any("REFUSED" in s for s in said) and said.count("71483.511 [UPDATER] FETCH: 64 OF 104") == 1, said
    # Without the anchor, a line of the same uptime as the last read is still new unless it was read.
    line, said = follow(["5.100 a\n5.170 [UPDATER] RECORD\n", "5.170 [UPDATER] RECORD\n5.170 [UPDATER] SLOT A CONFIRMED ON THE DISK\n"], before="5.000 y\n")
    assert line == "5.170 [UPDATER] SLOT A CONFIRMED ON THE DISK" and said.count("5.170 [UPDATER] RECORD") == 1, (line, said)


if __name__ == "__main__":
    for name, test in list(globals().items()):
        if name.startswith("test_"):
            test()
            print(f"ok {name}")
    print("link_update_test: all passed")
