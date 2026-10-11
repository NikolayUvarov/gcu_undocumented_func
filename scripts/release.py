#!/usr/bin/env python3
"""Releases: staging, publishing, checking (351-UPD-0005, docs/update/publishing.md; MC-9.2, 9.6, 9.7).

A release is a version number, the boot manifest of each architecture (350-UPD-0002) and every file those manifests
list, stored once by content. A channel file names a channel's latest version, the minimum version a device may run,
an expiry, and the SHA-256 of each architecture's manifest. It is signed with the release key, which is not the key
that signs boot manifests (MC-9.6).

Server layout:
    blobs/<sha256>                          every file of every release, by its SHA-256
    releases/<version>/<arch>/MANIFEST      the boot manifest and its signature, as the build made them
    releases/<version>/<arch>/MANIFEST.SIG
    channels/<channel>                      the channel: one line of JSON, then `ed25519 <signature in hex>`

Usage:
    release.py stage VERSION OUT [--arch x86_64=usb_root] [--arch aarch64=aarch64_root]
    release.py publish STAGED DEST [--channel stable] [--minimum N] [--days 30]   (DEST: a directory or host:dir)
    release.py check DEST [--channel stable] [--now ISO-8601]
    release.py --public                     the public test release key

Keys: the release seed from $MIND_RELEASE_KEY (a file of 64 hex digits, outside the repository) or the public test
release key; the boot key as sign_manifest.py reads it. With the test keys a layout is checked for accident, not
attack.
"""
import datetime
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import sign_manifest as sm  # noqa: E402

TEST_RELEASE_SEED = hashlib.sha256(b"MIND Core test release key: public, never for a release").digest()
ARCHES = {"x86_64": "usb_root", "aarch64": "aarch64_root"}


class ReleaseError(Exception):
    pass


def release_seed():
    path = os.environ.get("MIND_RELEASE_KEY")
    if not path:
        return TEST_RELEASE_SEED
    seed = bytes.fromhex(Path(path).read_text().strip())
    if len(seed) != 32:
        raise ReleaseError(f"{path}: a key is 32 bytes in hex")
    return seed


def manifest_files(text):
    """(path, size, sha256) of every file line of a boot manifest."""
    out = []
    for line in text.decode().splitlines():
        if line.startswith("file "):
            _, name, size, digest, _, _ = line.split(" ")
            out.append((name, int(size), digest))
    return out


def stage(version, out, volumes, boot_public=None):
    """Copies each volume's manifest and listed files into `out` as release `version`; checks each volume first."""
    if version < 1:
        raise ReleaseError("a version is a positive integer")
    out = Path(out)
    boot_public = boot_public or sm.public_key(sm.signing_seed())
    for arch, volume in volumes.items():
        volume = Path(volume)
        reason = sm.verify_volume(volume, boot_public)
        if reason:
            raise ReleaseError(f"{arch}: {reason}")
        text = (volume / "MANIFEST").read_bytes()
        where = out / "releases" / str(version) / arch
        where.mkdir(parents=True, exist_ok=False)
        shutil.copyfile(volume / "MANIFEST", where / "MANIFEST")
        shutil.copyfile(volume / "MANIFEST.SIG", where / "MANIFEST.SIG")
        (out / "blobs").mkdir(exist_ok=True)
        for name, _, digest in manifest_files(text):
            shutil.copyfile(volume / name, out / "blobs" / digest)
    (out / "VERSION").write_text(f"{version}\n")
    return out


def channel_bytes(channel, version, minimum, expires, manifests):
    """The channel file's one encoding: sorted keys, no spaces, a final newline."""
    if not 1 <= minimum <= version:
        raise ReleaseError("the minimum version is at least 1 and at most the version")
    body = {"channel": channel, "version": version, "minimum": minimum, "expires": expires, "manifests": dict(sorted(manifests.items()))}
    return (json.dumps(body, sort_keys=True, separators=(",", ":")) + "\n").encode()


# What a channel's fields may hold, as mind::release reads them on the device (libmind/src/release.rs).
NAME_MAX, ARCH_MAX, ARCHES_MAX = 32, 16, 4
WORD = re.compile(r"[A-Za-z0-9._-]+")
STAMP = re.compile(r"\d{4}-\d\d-\d\dT\d\d:\d\d:\d\dZ")
DIGEST = re.compile(r"[0-9a-f]{64}")


def channel_from(body, name):
    """The channel a signed body holds, or None and the reason (351-UPD-0014): UTF-8 JSON of the five fields, each of
    its type and range, in the one encoding. It never raises, whatever the body."""
    try:
        c = json.loads(body.decode("utf-8"))
    except (UnicodeDecodeError, ValueError, RecursionError):
        return None, "not UTF-8 JSON"
    if not isinstance(c, dict) or set(c) != {"channel", "version", "minimum", "expires", "manifests"}:
        return None, "not the fields of a channel"
    word = lambda v, most: isinstance(v, str) and len(v) <= most and WORD.fullmatch(v) is not None
    if not word(c["channel"], NAME_MAX) or c["channel"] != name:
        return None, "another channel"
    if not all(type(c[k]) is int for k in ("version", "minimum")) or not 1 <= c["minimum"] <= c["version"] < 2**64:
        return None, "version and minimum not with 1 <= minimum <= version"
    try:
        if not isinstance(c["expires"], str) or not STAMP.fullmatch(c["expires"]):
            raise ValueError
        datetime.datetime.strptime(c["expires"], "%Y-%m-%dT%H:%M:%SZ")
    except ValueError:
        return None, "expires not a time as YYYY-MM-DDTHH:MM:SSZ"
    m = c["manifests"]
    if not isinstance(m, dict) or not 1 <= len(m) <= ARCHES_MAX or not all(word(a, ARCH_MAX) and isinstance(d, str) and DIGEST.fullmatch(d) for a, d in m.items()):
        return None, f"manifests not 1 to {ARCHES_MAX} architectures with a SHA-256 each"
    if body != channel_bytes(c["channel"], c["version"], c["minimum"], c["expires"], m):
        return None, "not in its one encoding"
    return c, None


class Local:
    """A destination directory on this machine."""
    def __init__(self, root):
        self.root = Path(root)

    def read(self, path):
        p = self.root / path
        return p.read_bytes() if p.exists() else None

    def put_tree(self, source, path, replace):
        target = self.root / path
        target.mkdir(parents=True, exist_ok=True)
        for file in sorted(Path(source).rglob("*")):
            if file.is_file():
                dest = target / file.relative_to(source)
                if dest.exists() and not replace:
                    continue
                dest.parent.mkdir(parents=True, exist_ok=True)
                temporary = dest.with_name(dest.name + ".part")
                shutil.copyfile(file, temporary)
                os.replace(temporary, dest)

    def put_file(self, data, path):
        dest = self.root / path
        dest.parent.mkdir(parents=True, exist_ok=True)
        temporary = dest.with_name(dest.name + ".part")
        temporary.write_bytes(data)
        os.replace(temporary, dest)


class Remote:
    """A directory on a server reached with the system's OpenSSH: `host:dir` (rsync -e ssh)."""
    def __init__(self, target):
        self.host, self.dir = target.split(":", 1)

    def read(self, path):
        done = subprocess.run(["ssh", self.host, "cat", f"{self.dir}/{path}"], capture_output=True)
        return done.stdout if done.returncode == 0 else None

    def put_tree(self, source, path, replace):
        args = ["rsync", "-r", "-e", "ssh", *([] if replace else ["--ignore-existing"]), f"{source}/", f"{self.host}:{self.dir}/{path}/"]
        subprocess.run(["ssh", self.host, "mkdir", "-p", f"{self.dir}/{path}"], check=True)
        subprocess.run(args, check=True)

    def put_file(self, data, path):
        with tempfile.NamedTemporaryFile() as f:
            f.write(data)
            f.flush()
            subprocess.run(["rsync", "-e", "ssh", f.name, f"{self.host}:{self.dir}/{path}.part"], check=True)
        subprocess.run(["ssh", self.host, "mv", f"{self.dir}/{path}.part", f"{self.dir}/{path}"], check=True)


def destination(dest):
    return Remote(dest) if ":" in str(dest) and not Path(dest).exists() else Local(dest)


def publish(staged, dest, channel="stable", minimum=None, days=30, seed=None, now=None, upload=None):
    """Uploads the staged release, then the signed channel file: blobs first, manifests next, the channel last, so a
    reader never finds a channel naming what is not there. Refuses a version not above the channel's."""
    staged, target = Path(staged), upload or destination(dest)
    version = int((staged / "VERSION").read_text())
    current = target.read(f"channels/{channel}")
    published = 0
    if current is not None:
        c, why = channel_from(current.split(b"\n")[0] + b"\n", channel)
        if c is None:
            raise ReleaseError(f"channels/{channel} on the server: {why}")
        published = c["version"]
    if published >= version:
        raise ReleaseError(f"version {version} is not above the published {published}")
    if target.read(f"releases/{version}/x86_64/MANIFEST") or target.read(f"releases/{version}/aarch64/MANIFEST"):
        raise ReleaseError(f"version {version} was published before")
    manifests = {arch.name: hashlib.sha256((arch / "MANIFEST").read_bytes()).hexdigest() for arch in sorted((staged / "releases" / str(version)).iterdir())}
    now = now or datetime.datetime.now(datetime.timezone.utc)
    expires = (now + datetime.timedelta(days=days)).strftime("%Y-%m-%dT%H:%M:%SZ")
    body = channel_bytes(channel, version, minimum or version, expires, manifests)
    signature = sm.sign(seed or release_seed(), body)
    target.put_tree(staged / "blobs", "blobs", replace=False)
    target.put_tree(staged / "releases" / str(version), f"releases/{version}", replace=False)
    # The channel and its signature are one file, replaced by one rename: a reader sees the old pair or the new.
    target.put_file(body + b"ed25519 " + signature.hex().encode() + b"\n", f"channels/{channel}")
    return body


def check(dest, channel="stable", release_public=None, boot_public=None, now=None):
    """None if the channel's signature, every manifest it names and every blob they list verify; else the reason."""
    target = destination(dest)
    release_public = release_public or sm.public_key(release_seed())
    boot_public = boot_public or sm.public_key(sm.signing_seed())
    whole = target.read(f"channels/{channel}")
    if whole is None:
        return f"channels/{channel}: missing"
    lines = whole.split(b"\n")
    if len(lines) != 3 or lines[2] != b"" or not lines[1].startswith(b"ed25519 "):
        return f"channels/{channel}: not a signed channel"
    body = lines[0] + b"\n"
    try:
        signature = bytes.fromhex(lines[1][8:].decode())
    except ValueError:
        return f"channels/{channel}: not a signed channel"
    if not sm.verify(release_public, body, signature):
        return f"channels/{channel}: bad signature"
    c, why = channel_from(body, channel)
    if c is None:
        return f"channels/{channel}: {why}"
    now = now or datetime.datetime.now(datetime.timezone.utc)
    if datetime.datetime.strptime(c["expires"], "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=datetime.timezone.utc) <= now:
        return f"channels/{channel}: expired at {c['expires']}"
    for arch, digest in c["manifests"].items():
        text = target.read(f"releases/{c['version']}/{arch}/MANIFEST")
        sig = target.read(f"releases/{c['version']}/{arch}/MANIFEST.SIG")
        if text is None or hashlib.sha256(text).hexdigest() != digest:
            return f"releases/{c['version']}/{arch}/MANIFEST: not the one the channel names"
        if sig is None or not sm.verify(boot_public, text, sig):
            return f"releases/{c['version']}/{arch}/MANIFEST: bad signature"
        for name, size, blob in manifest_files(text):
            data = target.read(f"blobs/{blob}")
            if data is None or len(data) != size or hashlib.sha256(data).hexdigest() != blob:
                return f"blobs/{blob} ({arch} {name}): missing or not as listed"
    return None


def main(argv):
    def option(name, default=None, many=False):
        values = [argv[i + 1] for i, a in enumerate(argv[:-1]) if a == name]
        return values if many else (values[-1] if values else default)
    try:
        if argv[:1] == ["--public"]:
            print(sm.public_key(release_seed()).hex())
        elif argv[:1] == ["stage"] and len(argv) >= 3:
            arches = option("--arch", many=True) or [f"{a}={v}" for a, v in ARCHES.items() if (sm.ROOT / v / "MANIFEST").exists()]
            volumes = {a: (sm.ROOT / v if not Path(v).is_absolute() else Path(v)) for a, v in (x.split("=") for x in arches)}
            out = stage(int(argv[1]), argv[2], volumes)
            print(f"STAGED VERSION {argv[1]} IN {out}: {', '.join(volumes)}")
        elif argv[:1] == ["publish"] and len(argv) >= 3:
            minimum = option("--minimum")
            body = publish(argv[1], argv[2], option("--channel", "stable"), int(minimum) if minimum else None, int(option("--days", "30")))
            print(f"PUBLISHED {body.decode().strip()}" + (" (THE TEST RELEASE KEY)" if release_seed() == TEST_RELEASE_SEED else ""))
        elif argv[:1] == ["check"] and len(argv) >= 2:
            now = option("--now")
            reason = check(argv[1], option("--channel", "stable"), now=datetime.datetime.fromisoformat(now) if now else None)
            print(reason or "VERIFIED")
            return 1 if reason else 0
        else:
            print(__doc__)
            return 2
    except ReleaseError as e:
        print(f"release: {e}")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
