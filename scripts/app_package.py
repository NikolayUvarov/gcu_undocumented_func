#!/usr/bin/env python3
"""Builds, signs and checks a signed application package (750-UPD-0019, contract draft 0.1).

A package is a directory: the application's files, MANIFEST (format 1: one encoding, checked by the device's `apppkg`)
and MANIFEST.SIG, an Ed25519 signature over "MIND application package 1\\0" and MANIFEST's bytes, so it never passes as
a boot manifest's or a channel's. The key's role is "application": it signs nothing else.

Usage: app_package.py build DIR NAME VERSION ARCH SYSTEM   write DIR/MANIFEST and DIR/MANIFEST.SIG for DIR's other files,
                                                           signed with the key in $MIND_APP_KEY, or with the test key
       app_package.py check DIR PUBLIC_HEX [ARCH SYSTEM]   check a package as the device does
       app_package.py --public [KEY]                       print the public key (hex) of KEY or of the test key
The test key is public: it is derived from a fixed text, so anyone can sign with it. It is never on a device's list of
trusted application keys; a package signed with it is checked for accidental change, not for an attacker. A real key
is a file outside the repository holding the 32-byte seed in hex.
"""
import hashlib
import os
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from sign_manifest import public_key, requests, sign, verify  # noqa: E402

FORMAT_LINE = "MIND-APP-MANIFEST 1"
PREFIX = b"MIND application package 1\0"
TEST_SEED = hashlib.sha256(b"MIND Core test application key: public, never for a real package").digest()
MANIFEST_MAX, FILES_MAX, NAME_MAX, PATH_MAX = 64 * 1024, 256, 32, 128
ARCHES = ("x86_64", "aarch64")
NAME = re.compile(r"[a-z0-9][a-z0-9-]{0,31}")
PATH = re.compile(r"[a-z0-9._/-]{1,128}")
DECIMAL = re.compile(r"0|[1-9][0-9]{0,19}")


def signing_seed():
    """The seed of $MIND_APP_KEY (a file of 64 hex digits), or the test seed."""
    path = os.environ.get("MIND_APP_KEY")
    if not path:
        return TEST_SEED
    seed = bytes.fromhex(Path(path).read_text().strip())
    if len(seed) != 32:
        raise ValueError(f"{path}: a key is 32 bytes in hex")
    return seed


def path_ok(path):
    return PATH.fullmatch(path) is not None and all(part not in ("", ".", "..") for part in path.split("/"))


def manifest(directory, name, version, arch, system, public):
    """MANIFEST's bytes for every file under `directory` but MANIFEST and MANIFEST.SIG, sorted by path."""
    if not NAME.fullmatch(name) or not 1 <= version < 2 ** 64 or arch not in ARCHES or not 0 <= system < 2 ** 64:
        raise ValueError("a name of a-z, 0-9 and '-', a version of 1 or more, x86_64 or aarch64, a system of 0 or more")
    lines = [FORMAT_LINE, f"key {hashlib.sha256(public).hexdigest()[:16]}", f"name {name}", f"version {version}", f"arch {arch}", f"system {system}"]
    paths = sorted(p.relative_to(directory).as_posix() for p in directory.rglob("*") if p.is_file() and p.name not in ("MANIFEST", "MANIFEST.SIG"))
    if not paths or len(paths) > FILES_MAX:
        raise ValueError(f"1 to {FILES_MAX} files")
    for path in paths:
        if not path_ok(path):
            raise ValueError(f"{path}: a path of a-z, 0-9, '.', '_', '-' and '/', at most {PATH_MAX} bytes")
        data = (directory / path).read_bytes()
        flags, memory = requests(data)
        lines.append(f"file {path} {len(data)} {hashlib.sha256(data).hexdigest()} {flags:08x} {memory}")
    text = ("\n".join(lines) + "\n").encode()
    if len(text) > MANIFEST_MAX:
        raise ValueError(f"MANIFEST over {MANIFEST_MAX} bytes")
    return text


def build(directory, name, version, arch, system, seed=None):
    """Writes the package's MANIFEST and MANIFEST.SIG; returns the manifest."""
    directory, seed = Path(directory), seed or signing_seed()
    text = manifest(directory, name, version, arch, system, public_key(seed))
    (directory / "MANIFEST").write_bytes(text)
    (directory / "MANIFEST.SIG").write_bytes(sign(seed, PREFIX + text))
    return text


def check(directory, public, arch=None, system=None):
    """What is wrong with a package, as the device would say, or None."""
    directory = Path(directory)
    try:
        text, signature = (directory / "MANIFEST").read_bytes(), (directory / "MANIFEST.SIG").read_bytes()
    except OSError as error:
        return f"UNREADABLE: {error}"
    if len(text) > MANIFEST_MAX:
        return "TOO LARGE"
    lines = text.decode("ascii", "replace").split("\n")
    if lines[-1] != "" or any(not line or any(not 0x20 <= ord(c) < 0x7F for c in line) for line in lines[:-1]):
        return "MALFORMED: not lines of printable ASCII, each ended by LF"
    lines = lines[:-1]
    header = [FORMAT_LINE, "key ", "name ", "version ", "arch ", "system "]
    if len(lines) < 7 or lines[0] != FORMAT_LINE or any(not lines[i].startswith(header[i]) for i in range(1, 6)):
        return "MALFORMED: the header"
    key, name, version, built_for, needs = (lines[i].split(" ", 1)[1] for i in range(1, 6))
    if key != hashlib.sha256(public).hexdigest()[:16]:
        return "UNTRUSTED: another key"
    if not verify(public, PREFIX + text, signature):
        return "SIGNATURE"
    if not NAME.fullmatch(name) or not DECIMAL.fullmatch(version) or version == "0" or built_for not in ARCHES or not DECIMAL.fullmatch(needs):
        return "MALFORMED: a header field"
    files = lines[6:]
    if len(files) > FILES_MAX:
        return "TOO LARGE"
    previous = None
    for line in files:
        words = line.split(" ")
        if len(words) != 6 or words[0] != "file" or not path_ok(words[1]) or not DECIMAL.fullmatch(words[2]) or not re.fullmatch(r"[0-9a-f]{64}", words[3]) \
                or not re.fullmatch(r"[0-9a-f]{8}", words[4]) or not DECIMAL.fullmatch(words[5]) or (previous is not None and previous >= words[1]):
            return f"MALFORMED: {line[:60]}"
        previous = words[1]
        try:
            data = (directory / words[1]).read_bytes()
        except OSError:
            return f"MISSING: {words[1]}"
        if len(data) != int(words[2]) or hashlib.sha256(data).hexdigest() != words[3]:
            return f"CHANGED: {words[1]}"
    if arch is not None and built_for != arch:
        return "ANOTHER ARCHITECTURE"
    if system is not None and int(needs) > system:
        return "NEEDS A NEWER RELEASE"
    return None


def main(argv):
    if argv[:1] == ["--public"]:
        seed = bytes.fromhex(Path(argv[1]).read_text().strip()) if len(argv) > 1 else TEST_SEED
        print(public_key(seed).hex())
    elif argv[:1] == ["build"] and len(argv) == 6:
        seed = signing_seed()
        text = build(argv[1], argv[2], int(argv[3]), argv[4], int(argv[5]), seed)
        files, key = text.count(b"\nfile "), hashlib.sha256(public_key(seed)).hexdigest()[:16]
        print(f"SIGNED {argv[1]}/MANIFEST: {argv[2]} {argv[3]} {argv[4]}, {files} FILES, KEY {key}" + (" (THE TEST KEY)" if seed == TEST_SEED else ""))
    elif argv[:1] == ["check"] and len(argv) in (3, 5):
        reason = check(argv[1], bytes.fromhex(argv[2]), *((argv[3], int(argv[4])) if len(argv) == 5 else ()))
        print(reason or "VERIFIED")
        return 1 if reason else 0
    else:
        print(__doc__)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
