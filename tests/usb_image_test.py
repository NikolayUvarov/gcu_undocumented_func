#!/usr/bin/env python3
"""One make_usb_image.py call on a clean tree packs every program its build made (175-PRT-0007, audit A01).

The packager runs in a temporary tree whose 02_build.sh is a stand-in making the kernel, the boot services and the
programs named in programs.txt. The image is then read back against everything the build made, not against the
packager's own list. Needs qemu-img (the packager's FAT writer). --packager tests another make_usb_image.py.
"""
import argparse
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
from scripts.make_usb_image import check_image  # reads an image; the packager under test runs in its own tree

BUILD = '''#!/usr/bin/env bash
# A stand-in for 02_build.sh: the files a build puts in usb_root/, the programs from programs.txt.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"
exec python3 build_stub.py
'''
BUILD_STUB = r'''
from pathlib import Path
import re
import shutil
root = Path(__file__).resolve().parent
out = root / "usb_root"
shutil.rmtree(out, ignore_errors=True)
def put(name, data):
    (out / name).parent.mkdir(parents=True, exist_ok=True)
    (out / name).write_bytes(data)
def elf(name):
    return b"\x7fELF\x02\x01" + bytes(12) + b">\x00" + name.encode().ljust(64, b"\0")
boot = re.findall(r'"([\w-]+\.elf)"', re.search(r"BOOT_FILES[^=]*=\s*\[(.*?)\];", (root / "common/abi.rs").read_text(), re.S)[1])
programs = (root / "programs.txt").read_text().split()
put("EFI/BOOT/BOOTX64.EFI", b"MZ" + bytes(62))
for name in ["kernel.elf", *boot, *programs]:
    put(name, elf(name))
for licence in [root / "LICENSE-MIT", root / "LICENSE-APACHE", root / "THIRD_PARTY.md", *(root / "LICENSES").glob("*.txt")]:
    put(f"LICENSES/{licence.name}", licence.read_bytes())
put("voice/model.bin", b"model")
put("voice/commands.txt", b"commands\n")
put("MANIFEST", "".join(f"file {name} 0\n" for name in ["kernel.elf", *boot]).encode())
put("MANIFEST.SIG", b"signature")
'''


def built(tree):
    """Everything the stand-in build made, as the image must hold it."""
    out = tree / "usb_root"
    return {p.relative_to(out).as_posix(): p.read_bytes() for p in sorted(out.rglob("*")) if p.is_file()}


def package(tree, packager, qemu_img):
    subprocess.run([sys.executable, str(tree / "scripts/make_usb_image.py"), "--force", "--qemu-img", qemu_img],
                   cwd=tree, check=True, capture_output=True, text=True)
    image = tree / "dist/mind-core-usb.img"
    made = built(tree)
    check_image(image, made)  # raises ValueError naming the first file the image lacks
    return sorted(name for name in made if name.endswith(".elf"))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--packager", type=Path, default=ROOT / "scripts/make_usb_image.py")
    args = parser.parse_args()
    qemu_img = shutil.which("qemu-img")
    if not qemu_img:
        sys.exit("FAIL: qemu-img is needed (qemu-utils)")
    with tempfile.TemporaryDirectory(prefix="mind-image-test-") as temporary:
        tree = Path(temporary)
        (tree / "scripts").mkdir()
        shutil.copyfile(args.packager, tree / "scripts/make_usb_image.py")
        (tree / "common").mkdir()
        shutil.copyfile(ROOT / "common/abi.rs", tree / "common/abi.rs")
        shutil.copytree(ROOT / "LICENSES", tree / "LICENSES")
        for name in ("LICENSE-MIT", "LICENSE-APACHE", "THIRD_PARTY.md"):
            shutil.copyfile(ROOT / name, tree / name)
        (tree / "02_build.sh").write_text(BUILD)
        (tree / "build_stub.py").write_text(BUILD_STUB)
        # A clean tree: no usb_root/ before the one call that builds and packs.
        (tree / "programs.txt").write_text("shell.elf edit.elf\n")
        try:
            packed = package(tree, args.packager, qemu_img)
            assert "shell.elf" in packed and "edit.elf" in packed, packed
            print("PASS: a clean tree's first image holds every program its build made")
            # A program added to the build is in the next image.
            (tree / "programs.txt").write_text("shell.elf edit.elf added.elf\n")
            assert "added.elf" in package(tree, args.packager, qemu_img)
            print("PASS: a program added to the build is in the next image")
        except (ValueError, subprocess.CalledProcessError) as error:
            detail = getattr(error, "stderr", "") or ""
            sys.exit(f"FAIL: {error} {detail}".strip())


if __name__ == "__main__":
    main()
