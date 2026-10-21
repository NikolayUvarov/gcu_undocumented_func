#!/bin/bash
# Makes the archives tests/zip_host.rs reads (750-EFF-0005): with zip, 7-Zip and Python's zipfile, good and hostile.
# Run from this directory; the archives are committed so the host tests need none of these tools.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"
work=$(mktemp -d); trap 'rm -rf "$work"' EXIT
mkdir -p "$work/pkg/mindcore/x64"
printf '{"manifest_version":1,"package":{"name":"demo","version":"1.0.0"}}\n' > "$work/pkg/manifest.json"
printf 'MIND-PACKAGE 1\nname demo\nversion 1.0.0\n' > "$work/pkg/mindcore/x64/MANIFEST"
python3 -c "import random; random.seed(750); open('$work/pkg/mindcore/x64/demo.elf','wb').write(bytes(random.choice(b'MIND core ELF \x00\x01') for _ in range(20000)))"
rm -f ./*.zip
(cd "$work/pkg" && zip -q -X -r -9 "$OLDPWD/zip-deflate.zip" manifest.json mindcore)
(cd "$work/pkg" && 7z a -tzip -mx9 -bso0 -bsp0 "$OLDPWD/7z-deflate.zip" manifest.json mindcore)
(cd "$work/pkg" && zip -q -X -P secret "$OLDPWD/encrypted.zip" manifest.json)
python3 - <<'PY'
import zipfile, zlib
def stored(path, entries):
    with zipfile.ZipFile(path, "w", zipfile.ZIP_STORED) as z:
        for name, data in entries: z.writestr(zipfile.ZipInfo(name, (2026, 10, 21, 0, 0, 0)), data)
stored("stored.zip", [("manifest.json", b'{"manifest_version":1}\n'), ("mindcore/", b""), ("mindcore/x64/demo.elf", b"\x7fELF" + bytes(range(256)))])
for path, name in (("traversal.zip", "../evil.txt"), ("absolute.zip", "/etc/evil"), ("backslash.zip", "a\\evil"), ("drive.zip", "C:/evil"), ("dot.zip", "a/./b")):
    stored(path, [(name, b"evil\n")])
import warnings; warnings.simplefilter("ignore")
stored("duplicate.zip", [("a.txt", b"one\n"), ("a.txt", b"two\n")])
with zipfile.ZipFile("bomb.zip", "w", zipfile.ZIP_DEFLATED, compresslevel=9) as z:
    info = zipfile.ZipInfo("zeros.bin", (2026, 10, 21, 0, 0, 0)); info.compress_type = zipfile.ZIP_DEFLATED
    z.writestr(info, bytes(8 << 20))
# A stored file whose bytes no longer match the CRC-32 the directory names.
stored("crc.zip", [("data.bin", b"0123456789abcdef")])
b = bytearray(open("crc.zip", "rb").read()); b[b.index(b"0123456789abcdef")] ^= 1; open("crc.zip", "wb").write(b)
PY
ls -l ./*.zip
