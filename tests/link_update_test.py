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


if __name__ == "__main__":
    for name, test in list(globals().items()):
        if name.startswith("test_"):
            test()
            print(f"ok {name}")
    print("link_update_test: all passed")
