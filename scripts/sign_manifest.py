#!/usr/bin/env python3
"""Signs a boot volume (350-UPD-0002, docs/update/README.md; MC-9.1, 9.2, 3.11).

Writes VOLUME/MANIFEST, a versioned list of every shipped file with its size, SHA-256 and requested authorities, and
the build's inputs; and VOLUME/MANIFEST.SIG, an Ed25519 signature (RFC 8032) over the manifest's exact bytes. The
bootloader checks the signature against the public key built into it, and each image it loads against the manifest.
A manifest describes; it grants nothing (MC-3.11).

Usage: sign_manifest.py VOLUME            sign with the key in $MIND_BOOT_SIGNING_KEY, or with the test key
       sign_manifest.py --public [KEY]    print the public key (hex) of KEY or of the test key
       sign_manifest.py --verify VOLUME PUBLIC_HEX   check a volume as the bootloader does

The test key is public: it is derived from a fixed text, so anyone can sign with it. A volume signed with it is
checked for accidental change, not for an attacker. A release key is a file outside the repository holding the
32-byte seed in hex.
"""
import hashlib
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FORMAT = 1
TEST_SEED = hashlib.sha256(b"MIND Core test boot key: public, never for a release").digest()
REQUEST_MAGIC = b"MINDREQ1"

# Ed25519 (RFC 8032, section 5.1): edwards25519 in extended coordinates.
P = 2 ** 255 - 19
L = 2 ** 252 + 27742317777372353535851937790883648493
D = -121665 * pow(121666, P - 2, P) % P
SQRT_M1 = pow(2, (P - 1) // 4, P)


def _add(a, b):
    x1, y1, z1, t1 = a
    x2, y2, z2, t2 = b
    e, f = (y1 - x1) * (y2 - x2) % P, (y1 + x1) * (y2 + x2) % P
    g, h = 2 * t1 * t2 * D % P, 2 * z1 * z2 % P
    e, f, g, h = f - e, h - g, h + g, f + e
    return e * f % P, g * h % P, f * g % P, e * h % P


def _mul(s, point):
    q = (0, 1, 1, 0)
    while s > 0:
        if s & 1:
            q = _add(q, point)
        point = _add(point, point)
        s >>= 1
    return q


def _encode(point):
    x, y, z, _ = point
    zi = pow(z, P - 2, P)
    x, y = x * zi % P, y * zi % P
    return int.to_bytes(y | ((x & 1) << 255), 32, "little")


def _recover_x(y, sign):
    if y >= P:
        return None
    x2 = (y * y - 1) * pow(D * y * y + 1, P - 2, P)
    if x2 == 0:
        return None if sign else 0
    x = pow(x2, (P + 3) // 8, P)
    if (x * x - x2) % P:
        x = x * SQRT_M1 % P
    if (x * x - x2) % P:
        return None
    if (x & 1) != sign:
        x = P - x
    return x


def _decode(data):
    y = int.from_bytes(data, "little")
    sign, y = y >> 255, y & ((1 << 255) - 1)
    x = _recover_x(y, sign)
    return None if x is None else (x, y, 1, x * y % P)


def _equal(a, b):
    x1, y1, z1, _ = a
    x2, y2, z2, _ = b
    return (x1 * z2 - x2 * z1) % P == 0 and (y1 * z2 - y2 * z1) % P == 0


G = (15112221349535400772501151409588531511454012693041857206046113283949847762202,
     46316835694926478169428394003475163141307993866256225615783033603165251855960, 1,
     15112221349535400772501151409588531511454012693041857206046113283949847762202
     * 46316835694926478169428394003475163141307993866256225615783033603165251855960 % P)


def _expand(seed):
    h = hashlib.sha512(seed).digest()
    a = int.from_bytes(h[:32], "little")
    a &= (1 << 254) - 8
    a |= 1 << 254
    return a, h[32:]


def public_key(seed):
    return _encode(_mul(_expand(seed)[0], G))


def sign(seed, message):
    a, prefix = _expand(seed)
    public = _encode(_mul(a, G))
    r = int.from_bytes(hashlib.sha512(prefix + message).digest(), "little") % L
    big_r = _encode(_mul(r, G))
    k = int.from_bytes(hashlib.sha512(big_r + public + message).digest(), "little") % L
    return big_r + int.to_bytes((r + k * a) % L, 32, "little")


def verify(public, message, signature):
    if len(public) != 32 or len(signature) != 64:
        return False
    a, big_r = _decode(public), _decode(signature[:32])
    s = int.from_bytes(signature[32:], "little")
    if a is None or big_r is None or s >= L:
        return False
    k = int.from_bytes(hashlib.sha512(signature[:32] + public + message).digest(), "little") % L
    return _equal(_mul(s, G), _add(big_r, _mul(k, a)))


def signing_seed():
    """The seed of $MIND_BOOT_SIGNING_KEY (a file of 64 hex digits), or the test seed."""
    path = os.environ.get("MIND_BOOT_SIGNING_KEY")
    if not path:
        return TEST_SEED
    seed = bytes.fromhex(Path(path).read_text().strip())
    if len(seed) != 32:
        raise ValueError(f"{path}: a key is 32 bytes in hex")
    return seed


def shipped(volume):
    """The files a volume ships, by path: the bootloader, the kernel and every program, licences, the voice model."""
    names = []
    for path in sorted(volume.rglob("*")):
        rel = path.relative_to(volume).as_posix()
        if not path.is_file():
            continue
        top = rel.split("/")[0]
        if (rel.startswith("EFI/BOOT/") and rel.upper().endswith(".EFI")) or ("/" not in rel and rel.endswith(".elf")) or top in ("LICENSES", "voice"):
            names.append(rel)
    return names


def requests(data):
    """What a program's `.mind_request` section asks for (flags, MiB); 0, 0 for anything else."""
    if data[:4] != b"\x7fELF" or len(data) < 64:
        return 0, 0
    shoff, shentsize, shnum, shstrndx = int.from_bytes(data[0x28:0x30], "little"), int.from_bytes(data[0x3A:0x3C], "little"), int.from_bytes(data[0x3C:0x3E], "little"), int.from_bytes(data[0x3E:0x40], "little")
    if shentsize != 64 or not 0 < shnum <= 64 or shstrndx >= shnum or shoff + shnum * 64 > len(data):
        return 0, 0
    section = lambda i: data[shoff + i * 64:shoff + i * 64 + 64]
    strtab = section(shstrndx)
    names = data[int.from_bytes(strtab[0x18:0x20], "little"):][:int.from_bytes(strtab[0x20:0x28], "little")]
    for i in range(shnum):
        s = section(i)
        name = int.from_bytes(s[:4], "little")
        if names[name:name + 14] == b".mind_request\0":
            note = data[int.from_bytes(s[0x18:0x20], "little"):][:16]
            if note[:8] == REQUEST_MAGIC:
                return int.from_bytes(note[8:12], "little"), int.from_bytes(note[12:16], "little")
    return 0, 0


def build_inputs():
    """The commit, the toolchain, and one digest over every Cargo.lock: what the build was made from."""
    try:
        commit = subprocess.run(["git", "rev-parse", "HEAD"], cwd=ROOT, capture_output=True, text=True, check=True).stdout.strip()
    except (OSError, subprocess.CalledProcessError):
        commit = "unknown"
    toolchain = next((line.split("=")[1].strip().strip('"') for line in (ROOT / "rust-toolchain.toml").read_text().splitlines() if line.startswith("channel")), "unknown")
    locks = hashlib.sha256()
    for lock in sorted(ROOT.glob("*/Cargo.lock")):
        locks.update(f"{lock.relative_to(ROOT).as_posix()} {hashlib.sha256(lock.read_bytes()).hexdigest()}\n".encode())
    return commit, toolchain, locks.hexdigest()


def manifest(volume, public, inputs=None):
    commit, toolchain, locks = inputs or build_inputs()
    lines = [f"MIND-MANIFEST {FORMAT}", f"key {hashlib.sha256(public).hexdigest()[:16]}", f"commit {commit}", f"toolchain {toolchain}", f"inputs {locks}"]
    for name in shipped(volume):
        data = (volume / name).read_bytes()
        flags, memory = requests(data)
        lines.append(f"file {name} {len(data)} {hashlib.sha256(data).hexdigest()} {flags:08x} {memory}")
    return ("\n".join(lines) + "\n").encode()


def sign_volume(volume, seed=None, inputs=None):
    """Writes the volume's manifest and signature; returns the manifest."""
    volume = Path(volume)
    seed = seed or signing_seed()
    text = manifest(volume, public_key(seed), inputs)
    (volume / "MANIFEST").write_bytes(text)
    (volume / "MANIFEST.SIG").write_bytes(sign(seed, text))
    return text


def verify_volume(volume, public):
    """What the bootloader would say of the volume: None if every listed file matches, else the reason."""
    volume = Path(volume)
    text, signature = (volume / "MANIFEST").read_bytes(), (volume / "MANIFEST.SIG").read_bytes()
    if not verify(public, text, signature):
        return "MANIFEST: bad signature"
    for line in text.decode().splitlines()[5:]:
        _, name, size, digest, _, _ = line.split(" ")
        data = (volume / name).read_bytes() if (volume / name).exists() else None
        if data is None or len(data) != int(size) or hashlib.sha256(data).hexdigest() != digest:
            return f"{name}: not as the manifest says"
    return None


def main(argv):
    if argv[:1] == ["--public"]:
        seed = bytes.fromhex(Path(argv[1]).read_text().strip()) if len(argv) > 1 else TEST_SEED
        print(public_key(seed).hex())
    elif argv[:1] == ["--verify"] and len(argv) == 3:
        reason = verify_volume(argv[1], bytes.fromhex(argv[2]))
        print(reason or "VERIFIED")
        return 1 if reason else 0
    elif len(argv) == 1:
        seed = signing_seed()
        text = sign_volume(argv[0], seed)
        files = text.count(b"\nfile ")
        print(f"SIGNED {argv[0]}/MANIFEST: {files} FILES, KEY {hashlib.sha256(public_key(seed)).hexdigest()[:16]}" + (" (THE TEST KEY)" if seed == TEST_SEED else ""))
    else:
        print(__doc__)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
