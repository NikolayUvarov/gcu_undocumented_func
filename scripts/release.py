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
    channels/<channel>                      the channel: one line of JSON, then its release keys' signature lines
    keys/current                            the key file: each role's keys and threshold, signed by the root keys
                                                 (351-UPD-0026); a signature line is `ed25519 <key> <signature>` in hex
    keys/<version>                          every version of it, from 1: each signed by the root of the one before and
                                                 by its own (351-UPD-0027)

Usage:
    release.py stage VERSION OUT [--arch x86_64=usb_root] [--arch aarch64=aarch64_root]
    release.py publish STAGED DEST [--channel stable] [--minimum N] [--days 30]   (DEST: a directory or host:dir; the
                                                 minimum stays the published channel's, else 1: raise it only for a fix)
    release.py check DEST [--channel stable] [--now ISO-8601]
    release.py bundle DEST OUT [--channel stable] [--arch x86_64]   one ZIP of what a device needs of the channel
                                                 (751-UPD-0032), checked first: for the Effector gateway's install
    release.py --public                     the public test release key
    release.py keys make OUT --version N --role boot=KEY[,KEY…]:T --role recovery=… --role release=… --role timestamp=…
                         [--days 365]       a key file's body, unsigned: every role, its public keys and threshold
    release.py keys sign FILE SEED          adds one root key's signature line (SEED: a file of 64 hex digits)
    release.py keys check FILE [--root ROOT] [--now ISO-8601]   the root threshold, the form and the expiry
    release.py keys publish FILE DEST [--root ROOT]             checked against the root before it, then keys/<version>
                                                 and keys/current: the next version of the server's history (351-UPD-0027)
    release.py keys rotate FILE ROLE OUT --key KEY[,KEY…] --threshold T   the next version's body, ROLE given those keys
    release.py keys revoke FILE KEY OUT     the next version's body, KEY in no role
    release.py keys root SEED… --threshold T                    the root set's public file, $MIND_ROOT_KEYS's form
    release.py keys --test                  the test root set's public file

Keys: the release seeds from $MIND_RELEASE_KEY (files of 64 hex digits outside the repository, separated by the path
separator) or the public test release key; the boot key as sign_manifest.py reads it; the root set's public keys and
threshold from $MIND_ROOT_KEYS (`threshold T`, then a public key a line) or the test set, three keys of which two sign.
`publish` with the test release key puts the test key file on a server that has none. With the test keys a layout is
checked for accident, not attack.
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
import zipfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import sign_manifest as sm  # noqa: E402

TEST_RELEASE_SEED = hashlib.sha256(b"MIND Core test release key: public, never for a release").digest()
# The test root set (351-UPD-0026): three keys from fixed text, two of which sign; and the test keys of the roles that
# sign nothing yet.
TEST_ROOT_SEEDS = tuple(hashlib.sha256(f"MIND Core test root key {i}: public, never for a release".encode()).digest() for i in (1, 2, 3))
TEST_ROOT_THRESHOLD = 2
TEST_RECOVERY_SEED = hashlib.sha256(b"MIND Core test recovery key: public, never for a release").digest()
TEST_TIMESTAMP_SEED = hashlib.sha256(b"MIND Core test timestamp key: public, never for a release").digest()
ARCHES = {"x86_64": "usb_root", "aarch64": "aarch64_root"}
# A key file's roles in its order, the keys a role names and the signature lines a file carries, at most; the largest
# channel and key file (libmind/src/release.rs).
ROLES = ("boot", "recovery", "release", "root", "timestamp")  # root: the keys that sign the next key file (351-UPD-0027)
ROLE_KEYS, SIGNATURES_MAX = 4, 8
CHANNEL_MAX, KEYS_MAX = 2560, 4096


class ReleaseError(Exception):
    pass


def read_seed(path):
    try:
        seed = bytes.fromhex(Path(path).read_text().strip())
    except ValueError:
        seed = b""
    if len(seed) != 32:
        raise ReleaseError(f"{path}: a key is 32 bytes in hex")
    return seed


def release_seeds():
    """The seeds that sign a channel: $MIND_RELEASE_KEY's files, else the test release key."""
    paths = [p for p in os.environ.get("MIND_RELEASE_KEY", "").split(os.pathsep) if p]
    return [read_seed(p) for p in paths] or [TEST_RELEASE_SEED]


def release_seed():
    return release_seeds()[0]


def signature_lines(body, seeds):
    """One line `ed25519 <public key> <signature>` in hex for each seed, in order."""
    return b"".join(b"ed25519 " + sm.public_key(seed).hex().encode() + b" " + sm.sign(seed, body).hex().encode() + b"\n" for seed in seeds)


SIGNATURE_LINE = re.compile(rb"ed25519 ([0-9a-f]{64}) ([0-9a-f]{128})")


def split_signed(whole):
    """(body, [(public key, signature)]) of a file of one line and then 1 to SIGNATURES_MAX signature lines, or
    (None, the reason). A channel of before 351-UPD-0026 has one line `ed25519 <signature>`, which names no key."""
    if b"\n" not in whole or not whole.endswith(b"\n"):
        return None, "not a signed file"
    body, tail = whole.split(b"\n", 1)
    lines = tail[:-1].split(b"\n") if tail else []
    if not 1 <= len(lines) <= SIGNATURES_MAX:
        return None, f"not 1 to {SIGNATURES_MAX} signature lines"
    signatures = []
    for line in lines:
        m = SIGNATURE_LINE.fullmatch(line)
        if not m:
            old = re.fullmatch(rb"ed25519 [0-9a-f]{128}", line)
            return None, "a signature of before 351-UPD-0026, naming no key: sign it again" if old else "not a signature line"
        signatures.append((bytes.fromhex(m[1].decode()), bytes.fromhex(m[2].decode())))
    return (body + b"\n", signatures), None


def counted(keys, threshold, body, signatures):
    """How many of `keys` signed `body`: each key once, a line of another key passed over; and whether that is at least
    `threshold`."""
    good = {key for key, signature in signatures if key in set(keys) and sm.verify(key, body, signature)}
    return len(good), threshold >= 1 and len(good) >= threshold


def keys_bytes(version, expires, roles):
    """A key file's body in its one encoding: `roles` maps each of ROLES to (public keys, threshold)."""
    if set(roles) != set(ROLES):
        raise ReleaseError(f"a key file names every role: {', '.join(ROLES)}")
    out = {}
    for name in ROLES:
        keys, threshold = roles[name]
        keys = sorted(k.hex() if isinstance(k, bytes) else k for k in keys)
        if not 1 <= len(keys) <= ROLE_KEYS or len(set(keys)) != len(keys) or not all(DIGEST.fullmatch(k) for k in keys):
            raise ReleaseError(f"{name}: 1 to {ROLE_KEYS} distinct public keys in 64 lower-case hex digits")
        if not 1 <= threshold <= len(keys):
            raise ReleaseError(f"{name}: a threshold from 1 to its {len(keys)} keys")
        out[name] = {"keys": keys, "threshold": threshold}
    if not 1 <= version < 2**64:
        raise ReleaseError("a key file's version is a positive integer")
    body = {"expires": expires, "roles": out, "version": version}
    return (json.dumps(body, sort_keys=True, separators=(",", ":")) + "\n").encode()


def keys_from(body):
    """The key file a signed body holds, or None and the reason: UTF-8 JSON of its three fields, every role with its keys
    and threshold, in the one encoding. It never raises, whatever the body."""
    try:
        k = json.loads(body.decode("utf-8"))
    except (UnicodeDecodeError, ValueError, RecursionError):
        return None, "not UTF-8 JSON"
    if not isinstance(k, dict) or set(k) != {"expires", "roles", "version"} or not isinstance(k["roles"], dict) or set(k["roles"]) != set(ROLES):
        return None, f"not the fields of a key file, with the roles {', '.join(ROLES)}"
    if type(k["version"]) is not int or not 1 <= k["version"] < 2**64:
        return None, "a version not a positive integer"
    try:
        if not isinstance(k["expires"], str) or not STAMP.fullmatch(k["expires"]):
            raise ValueError
        datetime.datetime.strptime(k["expires"], "%Y-%m-%dT%H:%M:%SZ")
    except ValueError:
        return None, "expires not a time as YYYY-MM-DDTHH:MM:SSZ"
    roles = {}
    for name in ROLES:
        r = k["roles"][name]
        if not isinstance(r, dict) or set(r) != {"keys", "threshold"} or not isinstance(r["keys"], list) or type(r["threshold"]) is not int:
            return None, f"{name}: not keys and a threshold"
        if not all(isinstance(key, str) for key in r["keys"]):
            return None, f"{name}: a key not in hex"
        roles[name] = (r["keys"], r["threshold"])
    try:
        again = keys_bytes(k["version"], k["expires"], roles)
    except ReleaseError as e:
        return None, str(e)
    if body != again:
        return None, "not in its one encoding"
    k["roles"] = {name: ([bytes.fromhex(x) for x in keys], t) for name, (keys, t) in roles.items()}
    return k, None


def root_set(path=None):
    """(public keys, threshold) of the root set: the file at `path` or $MIND_ROOT_KEYS, else the test set."""
    path = path or os.environ.get("MIND_ROOT_KEYS")
    if not path:
        return [sm.public_key(seed) for seed in TEST_ROOT_SEEDS], TEST_ROOT_THRESHOLD
    lines = Path(path).read_text().split()
    if len(lines) < 3 or lines[0] != "threshold" or not lines[1].isdigit():
        raise ReleaseError(f"{path}: `threshold T`, then a public key a line")
    keys = lines[2:]
    if not all(DIGEST.fullmatch(k) for k in keys) or len(set(keys)) != len(keys) or not 1 <= int(lines[1]) <= len(keys) <= ROLE_KEYS:
        raise ReleaseError(f"{path}: 1 to {ROLE_KEYS} distinct public keys and a threshold up to their number")
    return [bytes.fromhex(k) for k in keys], int(lines[1])


def root_file(keys, threshold):
    """The root set's public file, as $MIND_ROOT_KEYS names one: `threshold T`, then the keys in order."""
    return f"threshold {threshold}\n" + "".join(f"{k.hex()}\n" for k in sorted(keys))


def check_keys(whole, root=None, now=None):
    """(the key file, None) if `whole` is a key file the threshold of the root set `root` trusts signed, and the threshold
    of its own root role (the next root, when it rotates it: 351-UPD-0027), and not expired; else (None, the reason)."""
    if len(whole) > KEYS_MAX:
        return None, f"larger than {KEYS_MAX} bytes"
    signed, why = split_signed(whole)
    if signed is None:
        return None, why
    body, signatures = signed
    k, why = keys_from(body)
    if k is None:
        return None, why
    keys, threshold = root or root_set()
    good, enough = counted(keys, threshold, body, signatures)
    if not enough:
        return None, f"signed by {good} of the root keys, not {threshold}"
    own, own_threshold = k["roles"]["root"]
    good, enough = counted(own, own_threshold, body, signatures)
    if not enough:
        return None, f"signed by {good} of its own root keys, not {own_threshold}"
    now = now or datetime.datetime.now(datetime.timezone.utc)
    if datetime.datetime.strptime(k["expires"], "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=datetime.timezone.utc) <= now:
        return None, f"expired at {k['expires']}"
    return k, None


def test_keys_file(now=None, days=3650, version=1, roots=TEST_ROOT_SEEDS[:TEST_ROOT_THRESHOLD], release=(TEST_RELEASE_SEED,),
                   root=(TEST_ROOT_SEEDS, TEST_ROOT_THRESHOLD)):
    """The test key file: the test boot, recovery and timestamp keys, the `release` seeds' keys, the `root` set (the
    test one), one of each other signing, signed by `roots`."""
    now = now or datetime.datetime.now(datetime.timezone.utc)
    expires = (now + datetime.timedelta(days=days)).strftime("%Y-%m-%dT%H:%M:%SZ")
    roles = {"boot": ([sm.public_key(sm.TEST_SEED)], 1), "recovery": ([sm.public_key(TEST_RECOVERY_SEED)], 1),
             "release": ([sm.public_key(seed) for seed in release], 1), "root": ([sm.public_key(seed) for seed in root[0]], root[1]),
             "timestamp": ([sm.public_key(TEST_TIMESTAMP_SEED)], 1)}
    body = keys_bytes(version, expires, roles)
    return body + signature_lines(body, roots)


def key_versions(target):
    """The versions the server's key history holds (`keys/<version>`), in order, from 1 up while they follow."""
    out, v = [], 1
    while target.read(f"keys/{v}") is not None:
        out.append(v)
        v += 1
    return out


def check_chain(target, root=None, now=None):
    """(the server's current key file, None) once every version from 1 up has been checked against the root before it,
    the first against the root set (351-UPD-0027), and `keys/current` is the last of them; else (None, the reason)."""
    current = target.read("keys/current")
    if current is None:
        return None, "keys/current: missing"
    trusted, k = root or root_set(), None
    for v in key_versions(target):
        k, why = check_keys(target.read(f"keys/{v}"), trusted, now)
        if k is None:
            return None, f"keys/{v}: {why}"
        if k["version"] != v:
            return None, f"keys/{v}: holds version {k['version']}"
        trusted = k["roles"]["root"]
    if k is None:
        return None, "keys/1: missing: the key history starts at version 1"
    if current != target.read(f"keys/{k['version']}"):
        return None, f"keys/current: not keys/{k['version']}, the last of the history"
    return k, None


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
        # The boot key vouches for the version, which the bootloader's floor reads (351-UPD-0024).
        if sm.manifest_version(text) != version:
            raise ReleaseError(f"{arch}: the manifest names version {sm.manifest_version(text)}, not {version}: sign it with --version {version}")
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


def server_keys(target, root=None, now=None, seeds=None):
    """The key file on the server, checked against the root set. A server without one gets the test key file when the
    channel is signed with the test release key alone."""
    whole = target.read("keys/current")
    if whole is None:
        if (seeds or release_seeds()) != [TEST_RELEASE_SEED] or root is not None or os.environ.get("MIND_ROOT_KEYS"):
            raise ReleaseError("keys/current: missing: publish a key file first (release.py keys publish)")
        whole = test_keys_file(now)
        target.put_file(whole, "keys/1")
        target.put_file(whole, "keys/current")
    k, why = check_chain(target, root, now)
    if k is None:
        raise ReleaseError(f"the server's key files: {why}")
    return k


def publish(staged, dest, channel="stable", minimum=None, days=30, seed=None, now=None, upload=None, root=None):
    """Uploads the staged release, then the signed channel file: blobs first, manifests next, the channel last, so a
    reader never finds a channel naming what is not there. Refuses a version not above the channel's, and release keys
    the server's key file does not name or that are fewer than its threshold."""
    staged, target = Path(staged), upload or destination(dest)
    seeds = [seed] if seed else release_seeds()
    k = server_keys(target, root, now, seeds)
    keys, threshold = k["roles"]["release"]
    named = [s for s in seeds if sm.public_key(s) in keys]
    if len(named) < len(seeds) or len(set(map(sm.public_key, named))) < threshold:
        raise ReleaseError(f"keys/current names {len(named)} of the {len(seeds)} release keys given, and its release role needs {threshold}")
    version = int((staged / "VERSION").read_text())
    current = target.read(f"channels/{channel}")
    published = 0
    if current is not None:
        c, why = channel_from(current.split(b"\n")[0] + b"\n", channel)
        if c is None:
            raise ReleaseError(f"channels/{channel} on the server: {why}")
        published = c["version"]
    # The minimum is the floor below which a device boots nothing (351-UPD-0024): it stays as published unless the
    # publisher raises it for a fix, so that an ordinary release can still be rolled back from.
    carried = c["minimum"] if current is not None else 1
    if published >= version:
        raise ReleaseError(f"version {version} is not above the published {published}")
    if target.read(f"releases/{version}/x86_64/MANIFEST") or target.read(f"releases/{version}/aarch64/MANIFEST"):
        raise ReleaseError(f"version {version} was published before")
    manifests = {arch.name: hashlib.sha256((arch / "MANIFEST").read_bytes()).hexdigest() for arch in sorted((staged / "releases" / str(version)).iterdir())}
    now = now or datetime.datetime.now(datetime.timezone.utc)
    expires = (now + datetime.timedelta(days=days)).strftime("%Y-%m-%dT%H:%M:%SZ")
    body = channel_bytes(channel, version, minimum or carried, expires, manifests)
    target.put_tree(staged / "blobs", "blobs", replace=False)
    target.put_tree(staged / "releases" / str(version), f"releases/{version}", replace=False)
    # The channel and its signatures are one file, replaced by one rename: a reader sees the old pair or the new.
    target.put_file(body + signature_lines(body, seeds), f"channels/{channel}")
    return body


def check(dest, channel="stable", boot_public=None, now=None, root=None):
    """None if the key file's root signatures, the channel's release signatures, every manifest it names and every blob
    they list verify; else the reason."""
    target = destination(dest)
    boot_public = boot_public or sm.public_key(sm.signing_seed())
    k, why = check_chain(target, root, now)
    if k is None:
        return why
    whole = target.read(f"channels/{channel}")
    if whole is None:
        return f"channels/{channel}: missing"
    if len(whole) > CHANNEL_MAX:
        return f"channels/{channel}: larger than {CHANNEL_MAX} bytes"
    signed, why = split_signed(whole)
    if signed is None:
        return f"channels/{channel}: {why}"
    body, signatures = signed
    keys, threshold = k["roles"]["release"]
    good, enough = counted(keys, threshold, body, signatures)
    if not enough:
        return f"channels/{channel}: signed by {good} of the release keys keys/current names, not {threshold}"
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


def bundle(dest, out, channel="stable", arches=None, now=None):
    """Writes OUT, one ZIP of stored entries holding the layout's paths of the key file, the channel, the manifests and signatures it
    names (of `arches`, else all) and the blobs they list, after `check` (751-UPD-0032). The device checks it as any
    source; the archive's own CRCs and dates mean nothing to it. Returns the paths, in the archive's order."""
    reason = check(dest, channel, now=now)
    if reason:
        raise ReleaseError(reason)
    target = destination(dest)
    whole = target.read(f"channels/{channel}")
    c, _ = channel_from(whole.split(b"\n")[0] + b"\n", channel)
    chosen = sorted(arches or c["manifests"])
    missing = [a for a in chosen if a not in c["manifests"]]
    if missing:
        raise ReleaseError(f"channels/{channel} names no manifest for {', '.join(missing)}")
    # The whole key history, for a device that holds an older version (351-UPD-0027), then the current one.
    paths, blobs = [*(f"keys/{v}" for v in key_versions(target)), "keys/current", f"channels/{channel}"], set()
    for arch in chosen:
        release = f"releases/{c['version']}/{arch}"
        paths += [f"{release}/MANIFEST", f"{release}/MANIFEST.SIG"]
        blobs |= {blob for _, _, blob in manifest_files(target.read(f"{release}/MANIFEST"))}
    paths += [f"blobs/{blob}" for blob in sorted(blobs)]
    write_bundle(out, [(path, target.read(path)) for path in paths])
    return paths


def write_bundle(out, entries):
    """A ZIP of stored entries, `(path, bytes)` in order, with fixed dates so that the same release makes the same file."""
    out = Path(out)
    part = out.with_name(out.name + ".part")
    with zipfile.ZipFile(part, "w", zipfile.ZIP_STORED) as archive:
        for path, data in entries:
            info = zipfile.ZipInfo(path, date_time=(1980, 1, 1, 0, 0, 0))
            info.external_attr = 0o644 << 16
            archive.writestr(info, data)
    part.replace(out)


def keys_command(argv, option):
    """`release.py keys …` (351-UPD-0026)."""
    root = root_set(option("--root")) if option("--root") else None
    if argv[:1] == ["--test"]:
        print(root_file([sm.public_key(seed) for seed in TEST_ROOT_SEEDS], TEST_ROOT_THRESHOLD), end="")
    elif argv[:1] == ["root"] and len(argv) >= 2:
        named = [a for i, a in enumerate(argv[1:], 1) if a != "--threshold" and argv[i - 1] != "--threshold"]
        seeds = [read_seed(a) for a in named]
        print(root_file([sm.public_key(seed) for seed in seeds], int(option("--threshold", "0"))), end="")
    elif argv[:1] == ["make"] and len(argv) >= 2:
        roles = {}
        for given in option("--role", many=True):
            name, _, rest = given.partition("=")
            keys, _, threshold = rest.rpartition(":")
            if name not in ROLES or not keys or not threshold.isdigit():
                raise ReleaseError(f"--role {given}: NAME=KEY[,KEY…]:THRESHOLD, NAME one of {', '.join(ROLES)}")
            roles[name] = (keys.split(","), int(threshold))
        days = int(option("--days", "365"))
        expires = (datetime.datetime.now(datetime.timezone.utc) + datetime.timedelta(days=days)).strftime("%Y-%m-%dT%H:%M:%SZ")
        body = keys_bytes(int(option("--version", "0")), expires, roles)
        Path(argv[1]).write_bytes(body)
        print(f"MADE KEY FILE VERSION {option('--version')} IN {argv[1]}, EXPIRES {expires}: UNSIGNED")
    elif argv[:1] == ["sign"] and len(argv) >= 3:
        whole, seed = Path(argv[1]).read_bytes(), read_seed(argv[2])
        body = whole.split(b"\n", 1)[0] + b"\n"
        k, why = keys_from(body)
        if k is None:
            raise ReleaseError(f"{argv[1]}: {why}")
        tail = whole[len(body):]
        signed, _ = split_signed(whole) if tail else ((body, []), None)
        if signed is None:
            raise ReleaseError(f"{argv[1]}: its signature lines are not in their form")
        if any(key == sm.public_key(seed) for key, _ in signed[1]):
            raise ReleaseError(f"{argv[1]}: this key signed it already")
        if len(signed[1]) == SIGNATURES_MAX:
            raise ReleaseError(f"{argv[1]}: {SIGNATURES_MAX} signatures already")
        Path(argv[1]).write_bytes(whole + signature_lines(body, [seed]))
        print(f"SIGNED {argv[1]} WITH {sm.public_key(seed).hex()[:16]}…: {len(signed[1]) + 1} SIGNATURES")
    elif argv[:1] == ["check"] and len(argv) >= 2:
        now = option("--now")
        k, why = check_keys(Path(argv[1]).read_bytes(), root, datetime.datetime.fromisoformat(now) if now else None)
        print(why or f"VERIFIED: KEY FILE VERSION {k['version']}, EXPIRES {k['expires']}")
        return 1 if why else 0
    elif argv[:1] == ["publish"] and len(argv) >= 3:
        # The next version of the server's history, checked against the root before it (351-UPD-0027): the root set for
        # version 1, else the last version's root role. The same file again is no change.
        whole, target = Path(argv[1]).read_bytes(), destination(argv[2])
        published = key_versions(target)
        if published and target.read(f"keys/{published[-1]}") == whole:
            print(f"KEY FILE VERSION {published[-1]} PUBLISHED ALREADY")
            return 0
        trusted = root or root_set()
        if published:
            last, why = check_chain(target, root)
            if last is None:
                raise ReleaseError(f"the server's key files: {why}")
            trusted = last["roles"]["root"]
        k, why = check_keys(whole, trusted)
        if k is None:
            raise ReleaseError(f"{argv[1]}: {why}")
        if k["version"] != len(published) + 1:
            raise ReleaseError(f"version {k['version']} does not follow the published {len(published) or 'none'}: the next is {len(published) + 1}")
        target.put_file(whole, f"keys/{k['version']}")
        target.put_file(whole, "keys/current")
        print(f"PUBLISHED KEY FILE VERSION {k['version']}, EXPIRES {k['expires']}")
    elif argv[:1] in (["rotate"], ["revoke"]) and len(argv) >= 4:
        # The next version's body from a key file (351-UPD-0027): `rotate FILE ROLE OUT --key K[,K…] --threshold T` gives
        # ROLE those keys; `revoke FILE KEY OUT` takes KEY out of every role. Then `keys sign` it, the root before and the
        # root after (when `root` rotates) each to its threshold.
        whole = Path(argv[1]).read_bytes()
        signed, why = split_signed(whole)
        k, why = keys_from(signed[0]) if signed else (None, why)
        if k is None:
            raise ReleaseError(f"{argv[1]}: {why}")
        roles = {name: (list(keys), t) for name, (keys, t) in k["roles"].items()}
        if argv[0] == "rotate":
            role, out = argv[2], argv[3]
            if role not in ROLES or not option("--key") or not (option("--threshold") or "").isdigit():
                raise ReleaseError(f"rotate FILE ROLE OUT --key K[,K…] --threshold T, ROLE one of {', '.join(ROLES)}")
            roles[role] = ([bytes.fromhex(x) for x in option("--key").split(",")], int(option("--threshold")))
        else:
            key, out = bytes.fromhex(argv[2]), argv[3]
            if not any(key in keys for keys, _ in roles.values()):
                raise ReleaseError(f"{argv[2]}: no role of {argv[1]} names it")
            roles = {name: ([x for x in keys if x != key], t) for name, (keys, t) in roles.items()}
            short = [name for name, (keys, t) in roles.items() if len(keys) < t]
            if short:
                raise ReleaseError(f"without it, {', '.join(short)}: fewer keys than the threshold; rotate instead")
        days = int(option("--days", "365"))
        expires = (datetime.datetime.now(datetime.timezone.utc) + datetime.timedelta(days=days)).strftime("%Y-%m-%dT%H:%M:%SZ")
        Path(out).write_bytes(keys_bytes(k["version"] + 1, expires, roles))
        print(f"MADE KEY FILE VERSION {k['version'] + 1} IN {out}, EXPIRES {expires}: UNSIGNED")
    else:
        print(__doc__)
        return 2
    return 0


def main(argv):
    def option(name, default=None, many=False):
        values = [argv[i + 1] for i, a in enumerate(argv[:-1]) if a == name]
        return values if many else (values[-1] if values else default)
    try:
        if argv[:1] == ["keys"]:
            return keys_command(argv[1:], option)
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
        elif argv[:1] == ["bundle"] and len(argv) >= 3:
            paths = bundle(argv[1], argv[2], option("--channel", "stable"), option("--arch", many=True) or None)
            print(f"BUNDLED {len(paths)} FILES OF channels/{option('--channel', 'stable')} INTO {argv[2]} ({Path(argv[2]).stat().st_size} BYTES)")
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
