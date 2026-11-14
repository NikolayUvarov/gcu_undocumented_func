#!/usr/bin/env python3
"""Tests of releases (scripts/release.py, issue 351-UPD-0005; MC-9.2, 9.4, 9.6): a staged release published to a
directory verifies; a version not above the channel's is refused; an upload cut before the channel leaves the old
channel whole; a changed channel, manifest or blob, an expired channel and a channel signed with the boot key are
refused by the checker; a signed body that is not a channel gets a reason, never an exception (351-UPD-0014); a bundle
holds what a device needs of the channel, stored, the same for the same release, and only of a layout that verifies
(751-UPD-0032). The key file (351-UPD-0026): made, signed a root key at a time and checked against the root threshold;
published only above the server's version; a channel signed by a key it does not name refused; real keys refused on a
server without one."""
import datetime
import hashlib
import json
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
import release  # noqa: E402
import sign_manifest as sm  # noqa: E402

NOW = datetime.datetime(2026, 10, 8, tzinfo=datetime.timezone.utc)


def volume(where, kernel, version=1):
    v = Path(where)
    (v / "EFI/BOOT").mkdir(parents=True)
    (v / "EFI/BOOT/BOOTX64.EFI").write_bytes(b"MZ loader")
    (v / "kernel.elf").write_bytes(kernel)
    (v / "shell.elf").write_bytes(b"\x7fELF the same in every release")
    sm.sign_volume(v, sm.TEST_SEED, ("c" * 40, "nightly-x", "d" * 64), version=version)
    return v


class Releases(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.dir = Path(self.temp.name)
        self.dest = self.dir / "server"
        self.dest.mkdir()

    def tearDown(self):
        self.temp.cleanup()

    def staged(self, version, kernel=None):
        v = volume(self.dir / f"volume{version}", kernel or f"\x7fELF kernel {version}".encode(), version)
        return release.stage(version, self.dir / f"staged{version}", {"x86_64": v})

    def check(self, **kw):
        return release.check(self.dest, now=NOW, **kw)

    def test_a_published_release_verifies(self):
        body = release.publish(self.staged(1), self.dest, now=NOW)
        self.assertIsNone(self.check())
        c = json.loads(body)
        self.assertEqual((c["version"], c["minimum"], c["expires"]), (1, 1, "2026-11-07T00:00:00Z"))
        # Files are stored once, by content: a second release adds only what changed.
        release.publish(self.staged(2), self.dest, now=NOW)
        self.assertIsNone(self.check())
        self.assertEqual(len(list((self.dest / "blobs").iterdir())), 4)
        # 351-UPD-0024: the minimum stays as published unless raised, so release 1 stays a rollback's target.
        self.assertEqual(json.loads((self.dest / "channels/stable").read_bytes().split(b"\n")[0])["minimum"], 1)
        release.publish(self.staged(3), self.dest, minimum=3, now=NOW)
        self.assertEqual(json.loads((self.dest / "channels/stable").read_bytes().split(b"\n")[0])["minimum"], 3)

    def test_a_manifest_of_another_version_is_not_staged(self):
        # 351-UPD-0024: the boot key vouches for the version the bootloader's floor reads.
        v = volume(self.dir / "v7", b"\x7fELF kernel", 7)
        with self.assertRaises(release.ReleaseError):
            release.stage(8, self.dir / "out8", {"x86_64": v})
        v0 = Path(self.dir / "v0")
        volume(v0, b"\x7fELF kernel")
        sm.sign_volume(v0, sm.TEST_SEED, ("c" * 40, "nightly-x", "d" * 64))
        with self.assertRaises(release.ReleaseError):
            release.stage(1, self.dir / "out0", {"x86_64": v0})

    def test_a_version_not_above_the_channel_is_refused(self):
        release.publish(self.staged(2), self.dest, now=NOW)
        for version in (1, 2):
            with self.assertRaises(release.ReleaseError):
                release.publish(self.staged(version) if version == 1 else self.dir / "staged2", self.dest, now=NOW)
        with self.assertRaises(release.ReleaseError):
            release.channel_bytes("stable", 3, 4, "2026-11-07T00:00:00Z", {})

    def test_an_upload_cut_before_the_channel_leaves_the_old_one_whole(self):
        release.publish(self.staged(1), self.dest, now=NOW)

        class Cut(release.Local):
            def put_file(self, data, path):
                raise OSError("the connection dropped")
        with self.assertRaises(OSError):
            release.publish(self.staged(2), self.dest, now=NOW, upload=Cut(self.dest))
        # Version 2's blobs and manifest arrived; the channel still names version 1, whole.
        self.assertTrue((self.dest / "releases/2/x86_64/MANIFEST").exists())
        self.assertIsNone(self.check())
        self.assertEqual(json.loads((self.dest / "channels/stable").read_bytes().split(b"\n")[0])["version"], 1)

    def test_changes_are_refused(self):
        release.publish(self.staged(1), self.dest, now=NOW)
        channel = self.dest / "channels/stable"
        original = channel.read_bytes()
        channel.write_bytes(original.replace(b'"minimum":1', b'"minimum":0'))
        self.assertEqual(self.check(), "channels/stable: signed by 0 of the release keys keys/current names, not 1")
        channel.write_bytes(original)
        self.assertEqual(release.check(self.dest, now=NOW + datetime.timedelta(days=31)), "channels/stable: expired at 2026-11-07T00:00:00Z")
        manifest = self.dest / "releases/1/x86_64/MANIFEST"
        text = manifest.read_bytes()
        manifest.write_bytes(text + b"\n")
        self.assertEqual(self.check(), "releases/1/x86_64/MANIFEST: not the one the channel names")
        manifest.write_bytes(text)
        blob = self.dest / "blobs" / hashlib.sha256(b"\x7fELF kernel 1").hexdigest()
        blob.write_bytes(b"\x7fELF kernel 9")
        self.assertIn("missing or not as listed", self.check())

    def test_a_signed_channel_of_another_shape_gets_a_reason(self):
        # 351-UPD-0014 (from 351-ASR-0006's fuzzing): a body signed with the release key, which anyone may do with the
        # public test key, but not a channel, is refused with a reason; none raises.
        release.publish(self.staged(1), self.dest, now=NOW)
        channel = self.dest / "channels/stable"
        good = json.loads(channel.read_bytes().split(b"\n")[0])
        def answer(body):
            if isinstance(body, dict):
                body = (json.dumps(body, sort_keys=True, separators=(",", ":")) + "\n").encode()
            channel.write_bytes(body + release.signature_lines(body, [release.release_seed()]))
            return self.check()
        cases = [
            (b"\xff\xfe not UTF-8\n", "not UTF-8 JSON"),
            (b"{not json\n", "not UTF-8 JSON"),
            (b"[" * 2300 + b"\n", "not UTF-8 JSON"),  # with its signature line, within CHANNEL_MAX
            (b"[" * 100000 + b"\n", f"larger than {release.CHANNEL_MAX} bytes"),
            ({k: v for k, v in good.items() if k != "expires"}, "not the fields of a channel"),
            ({**good, "minimum": [1]}, "version and minimum not with 1 <= minimum <= version"),
            ({**good, "version": True}, "version and minimum not with 1 <= minimum <= version"),
            ({**good, "minimum": 2}, "version and minimum not with 1 <= minimum <= version"),
            ({**good, "version": 2**64}, "version and minimum not with 1 <= minimum <= version"),
            ({**good, "manifests": 5}, "manifests not 1 to 4 architectures with a SHA-256 each"),
            ({**good, "manifests": {}}, "manifests not 1 to 4 architectures with a SHA-256 each"),
            ({**good, "manifests": {"x86_64": "A" * 64}}, "manifests not 1 to 4 architectures with a SHA-256 each"),
            ({**good, "expires": "2099-01-01"}, "expires not a time as YYYY-MM-DDTHH:MM:SSZ"),
            ({**good, "expires": "2026-02-30T00:00:00Z"}, "expires not a time as YYYY-MM-DDTHH:MM:SSZ"),
            ({**good, "expires": 2099}, "expires not a time as YYYY-MM-DDTHH:MM:SSZ"),
            ({**good, "channel": "beta"}, "another channel"),
            (json.dumps(good).encode() + b"\n", "not in its one encoding"),
        ]
        for body, why in cases:
            self.assertEqual(answer(body), f"channels/stable: {why}", body[:80] if isinstance(body, bytes) else body)

    def test_the_release_key_is_not_the_boot_key(self):
        # A channel signed with the boot key is refused: the keys have different purposes (MC-9.6). publish signs
        # with none the key file does not name, and the checker counts none.
        release.publish(self.staged(1), self.dest, now=NOW)
        with self.assertRaisesRegex(release.ReleaseError, "names 0 of the 1 release keys given"):
            release.publish(self.staged(2), self.dest, now=NOW, seed=sm.TEST_SEED)
        channel = self.dest / "channels/stable"
        body = channel.read_bytes().split(b"\n")[0] + b"\n"
        channel.write_bytes(body + release.signature_lines(body, [sm.TEST_SEED]))
        self.assertEqual(self.check(), "channels/stable: signed by 0 of the release keys keys/current names, not 1")
        self.assertNotEqual(sm.public_key(release.TEST_RELEASE_SEED), sm.public_key(sm.TEST_SEED))

    def test_a_volume_that_does_not_verify_is_not_staged(self):
        v = volume(self.dir / "bad", b"\x7fELF kernel")
        (v / "kernel.elf").write_bytes(b"\x7fELF kernel!")
        with self.assertRaises(release.ReleaseError):
            release.stage(1, self.dir / "out", {"x86_64": v})

    def test_a_release_served_over_https(self):
        # The test server: what a device would fetch is the published bytes, over TLS 1.3 with a CA the client trusts.
        import shutil
        import ssl
        import subprocess
        import urllib.request
        import serve_release
        if not shutil.which("openssl"):
            self.skipTest("openssl is not installed")
        release.publish(self.staged(1), self.dest, now=NOW)
        cert, key = self.dir / "cert.pem", self.dir / "key.pem"
        subprocess.run(["openssl", "req", "-x509", "-newkey", "ec", "-pkeyopt", "ec_paramgen_curve:P-256", "-nodes", "-days", "1",
                        "-subj", "/CN=localhost", "-addext", "subjectAltName=IP:127.0.0.1", "-keyout", str(key), "-out", str(cert)],
                       check=True, capture_output=True)
        server = serve_release.serve(self.dest, cert, key)
        try:
            url = f"https://127.0.0.1:{server.server_address[1]}/channels/stable"
            with urllib.request.urlopen(url, context=ssl.create_default_context(cafile=str(cert))) as reply:
                self.assertEqual(reply.read(), (self.dest / "channels/stable").read_bytes())
        finally:
            server.shutdown()

    def test_a_client_that_never_starts_its_handshake_holds_no_other(self):
        # 000-KRN-0210: the server's handshake ran in accept, so one connection that never sent its hello held every
        # other until it closed; a connection that abandons its handshake is closed alone.
        import shutil
        import socket
        import ssl
        import subprocess
        import urllib.request
        import serve_release
        if not shutil.which("openssl"):
            self.skipTest("openssl is not installed")
        (self.dest / "file").write_bytes(b"served\n")
        cert, key = self.dir / "cert.pem", self.dir / "key.pem"
        subprocess.run(["openssl", "req", "-x509", "-newkey", "ec", "-pkeyopt", "ec_paramgen_curve:P-256", "-nodes", "-days", "1",
                        "-subj", "/CN=localhost", "-addext", "subjectAltName=IP:127.0.0.1", "-keyout", str(key), "-out", str(cert)],
                       check=True, capture_output=True)
        server = serve_release.serve(self.dest, cert, key)
        url = f"https://127.0.0.1:{server.server_address[1]}/file"
        context = ssl.create_default_context(cafile=str(cert))
        try:
            with socket.create_connection(("127.0.0.1", server.server_address[1])), \
                 socket.create_connection(("127.0.0.1", server.server_address[1]), timeout=10) as plain:
                for _ in range(3):
                    with urllib.request.urlopen(url, context=context, timeout=10) as reply:
                        self.assertEqual(reply.read(), b"served\n")
                # A request without TLS fails its handshake: that connection is closed, and the server still serves.
                plain.sendall(b"GET /file HTTP/1.0\r\n\r\n")
                while plain.recv(64):  # an alert, perhaps; then the end, within the timeout
                    pass
                with urllib.request.urlopen(url, context=context, timeout=10) as reply:
                    self.assertEqual(reply.read(), b"served\n")
        finally:
            server.shutdown()

    def test_the_built_volumes(self):
        # The volumes the build signed, if they are built: staged, published and verified as a release.
        import shutil
        volumes = {a: ROOT / v for a, v in release.ARCHES.items() if (ROOT / v / "MANIFEST").exists()}
        if not volumes:
            self.skipTest("no built volume")
        copies = {}
        for a, v in volumes.items():  # signed again with the release's version, as scripts/link_update.py does
            copies[a] = self.dir / f"built-{a}"
            shutil.copytree(v, copies[a])
            sm.sign_volume(copies[a], version=1)
        release.publish(release.stage(1, self.dir / "built", copies), self.dest, now=NOW)
        self.assertIsNone(self.check())

    def test_a_bundle_holds_what_a_device_needs(self):
        release.publish(self.staged(1), self.dest, now=NOW)
        release.publish(self.staged(2), self.dest, now=NOW)
        out = self.dir / "bundle.zip"
        paths = release.bundle(self.dest, out, now=NOW)
        manifest = (self.dest / "releases/2/x86_64/MANIFEST").read_bytes()
        blobs = sorted(blob for _, _, blob in release.manifest_files(manifest))
        self.assertEqual(paths, ["keys/1", "keys/current", "channels/stable", "timestamps/stable", "releases/2/x86_64/MANIFEST", "releases/2/x86_64/MANIFEST.SIG", *(f"blobs/{b}" for b in blobs)])
        with zipfile.ZipFile(out) as archive:
            self.assertEqual(archive.namelist(), paths)
            for info in archive.infolist():
                self.assertEqual(info.compress_type, zipfile.ZIP_STORED, info.filename)
                self.assertEqual(archive.read(info), (self.dest / info.filename).read_bytes(), info.filename)
        # Release 1's kernel is not release 2's: only what the channel's manifest lists goes in.
        self.assertEqual(len(list((self.dest / "blobs").iterdir())), len(blobs) + 1)
        again = self.dir / "again.zip"
        release.bundle(self.dest, again, now=NOW)
        self.assertEqual(out.read_bytes(), again.read_bytes())
        with self.assertRaisesRegex(release.ReleaseError, "names no manifest for aarch64"):
            release.bundle(self.dest, self.dir / "other.zip", arches=["aarch64"], now=NOW)
        # A layout that does not verify is not bundled.
        blob = self.dest / "blobs" / blobs[0]
        blob.write_bytes(blob.read_bytes() + b"!")
        with self.assertRaisesRegex(release.ReleaseError, "missing or not as listed"):
            release.bundle(self.dest, self.dir / "bad.zip", now=NOW)
        self.assertFalse((self.dir / "bad.zip").exists())

    def keys(self, *argv):
        # release.py keys …, its exit status and what it printed.
        import contextlib, io
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            status = release.main(["keys", *map(str, argv)])
        return status, out.getvalue()

    def test_a_key_file_made_signed_and_checked(self):
        seeds = [self.dir / f"root{i}" for i in range(5)]
        for i, seed in enumerate(seeds):
            seed.write_text(hashlib.sha256(f"a root key of this test {i}".encode()).hexdigest())
        pub = [sm.public_key(release.read_seed(seed)).hex() for seed in seeds]
        root = self.dir / "root.pub"
        status, text = self.keys("root", *seeds[:3], "--threshold", 2)
        root.write_text(text)
        self.assertEqual((status, release.root_set(root)[1]), (0, 2))
        public = sm.public_key(release.TEST_RELEASE_SEED).hex()
        stamper = sm.public_key(release.TEST_TIMESTAMP_SEED).hex()
        boot = sm.public_key(sm.TEST_SEED).hex()  # the boot key that signs the staged manifests (351-UPD-0029)
        roles = [x for name in release.ROLES if name not in ("boot", "root", "timestamp") for x in ("--role", f"{name}={public}:1")]
        roles += ["--role", f"boot={boot}:1", "--role", f"root={','.join(pub[:3])}:2", "--role", f"timestamp={stamper}:1"]
        made = self.dir / "keys"
        self.assertEqual(self.keys("make", made, "--version", 1, *roles)[0], 0)
        self.assertIn("not 1 to 8 signature lines", self.keys("check", made, "--root", root)[1])
        self.keys("sign", made, seeds[0])
        # One root key of a 2-of-3 threshold: refused (351-UPD-0026's criterion); the same key again does not count twice.
        self.assertIn("signed by 1 of the root keys, not 2", self.keys("check", made, "--root", root)[1])
        self.assertIn("this key signed it already", self.keys("sign", made, seeds[0])[1])
        whole = made.read_bytes()
        made.write_bytes(whole + whole[whole.index(b"\n") + 1:])
        self.assertIn("signed by 1 of the root keys, not 2", self.keys("check", made, "--root", root)[1])
        made.write_bytes(whole)
        # A key outside the root set is passed over.
        (self.dir / "root9").write_text("ab" * 32)
        self.keys("sign", made, self.dir / "root9")
        self.assertIn("signed by 1 of the root keys, not 2", self.keys("check", made, "--root", root)[1])
        self.keys("sign", made, seeds[2])
        expires = json.loads(whole.split(b"\n")[0])["expires"]
        self.assertEqual(self.keys("check", made, "--root", root), (0, f"VERIFIED: KEY FILE VERSION 1, EXPIRES {expires}\n"))
        # Published as the history's first version; the same file again is no change; another version 1 is refused.
        self.assertEqual(self.keys("publish", made, self.dest, "--root", root)[0], 0)
        self.assertIn("PUBLISHED ALREADY", self.keys("publish", made, self.dest, "--root", root)[1])
        again = self.dir / "again"
        self.keys("make", again, "--version", 1, "--days", 30, *roles)
        for seed in seeds[:2]:
            self.keys("sign", again, seed)
        self.assertIn("version 1 does not follow the published 1: the next is 2", self.keys("publish", again, self.dest, "--root", root)[1])
        # A channel under it: the test release key is the release role's; checked against that root set alone.
        release.publish(self.staged(1), self.dest, now=NOW, root=release.root_set(root))
        self.assertIsNone(self.check(root=release.root_set(root)))
        self.assertEqual(self.check(), "keys/1: signed by 0 of the root keys, not 2")
        # The root rotated to keys 3 and 4, one of which signs (351-UPD-0027): the old root's threshold and the new one's
        # must both sign, and the next version is then checked against the new root alone.
        rotated = self.dir / "rotated"
        self.assertEqual(self.keys("rotate", made, "root", rotated, "--key", ",".join(pub[3:]), "--threshold", 1)[0], 0)
        self.keys("sign", rotated, seeds[0])
        self.keys("sign", rotated, seeds[3])
        self.assertIn("signed by 1 of the root keys, not 2", self.keys("publish", rotated, self.dest, "--root", root)[1])
        unsigned = rotated.read_bytes()
        self.keys("sign", rotated, seeds[1])
        self.assertEqual(self.keys("publish", rotated, self.dest, "--root", root)[0], 0)
        rotated.write_bytes(unsigned.split(b"\n")[0] + b"\n")
        for seed in seeds[:2]:
            self.keys("sign", rotated, seed)
        self.assertIn("signed by 0 of its own root keys, not 1", self.keys("check", rotated, "--root", root)[1])
        self.assertIsNone(self.check(root=release.root_set(root)))
        self.assertEqual(release.key_versions(release.destination(self.dest)), [1, 2])
        # Version 3 signed by the old root alone: refused, as that root is no longer trusted.
        third = self.dir / "third"
        self.keys("rotate", self.dest / "keys/2", "release", third, "--key", public, "--threshold", 1)
        for seed in seeds[:3]:
            self.keys("sign", third, seed)
        self.assertIn("signed by 0 of the root keys, not 1", self.keys("publish", third, self.dest, "--root", root)[1])
        # A revocation that would leave a role short of its threshold is refused; a history with a version missing fails.
        self.assertIn("without it, recovery, release: fewer keys than the threshold", self.keys("revoke", made, public, third)[1])
        (self.dest / "keys/1").unlink()
        self.assertEqual(self.check(root=release.root_set(root)), "keys/1: missing: the key history starts at version 1")

    def test_a_timestamp_renewed_without_the_release_key(self):
        # 351-UPD-0028: publish writes the timestamp last; `timestamp` renews it, one version more, with the timestamp
        # key alone; check refuses one expired, one the release key signed and one naming another channel file.
        import contextlib, io, os
        from unittest import mock
        release.publish(self.staged(1), self.dest, now=NOW)
        path = self.dest / "timestamps/stable"
        first = path.read_bytes()
        s, why = release.timestamp_from(first.split(b"\n")[0] + b"\n", "stable")
        channel = hashlib.sha256((self.dest / "channels/stable").read_bytes()).hexdigest()
        self.assertEqual((s["version"], s["digest"], s["expires"]), (1, channel, "2026-10-09T00:00:00Z"), why)
        out = io.StringIO()
        with mock.patch.dict(os.environ, {"MIND_RELEASE_KEY": str(self.dir / "no-such-key")}), contextlib.redirect_stdout(out):
            status = release.main(["timestamp", str(self.dest), "--hours", "6"])
        self.assertEqual(status, 0, out.getvalue())
        self.assertIn("TIMESTAMPED", out.getvalue())
        s, _ = release.timestamp_from(path.read_bytes().split(b"\n")[0] + b"\n", "stable")
        self.assertEqual((s["version"], s["digest"]), (2, channel))
        self.assertIsNone(self.check())
        path.write_bytes(first)
        self.assertEqual(release.check(self.dest, now=NOW + datetime.timedelta(days=2)), "timestamps/stable: expired at 2026-10-09T00:00:00Z")
        body = first.split(b"\n")[0] + b"\n"
        path.write_bytes(body + release.signature_lines(body, [release.TEST_RELEASE_SEED]))
        self.assertEqual(self.check(), "timestamps/stable: signed by 0 of the timestamp keys keys/current names, not 1")
        other = release.timestamp_bytes("stable", "00" * 32, "2026-10-09T00:00:00Z", 3)
        path.write_bytes(other + release.signature_lines(other, [release.TEST_TIMESTAMP_SEED]))
        self.assertEqual(self.check(), "timestamps/stable: names another channel file")
        path.unlink()
        self.assertEqual(self.check(), "timestamps/stable: missing")

    def test_a_recovery_order_with_the_recovery_key_alone(self):
        # 351-UPD-0030: `recover` writes recovery/<channel>, its sequence one more each time, signed by the recovery key
        # the key file names; another key is refused.
        import contextlib, io
        release.publish(self.staged(1), self.dest, now=NOW)
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            self.assertEqual(release.main(["recover", str(self.dest), "--version", "1", "--floor", "1"]), 0, out.getvalue())
            self.assertEqual(release.main(["recover", str(self.dest), "--version", "1", "--floor", "1", "--days", "2"]), 0, out.getvalue())
        whole = (self.dest / "recovery/stable").read_bytes()
        body, signatures = release.split_signed(whole)[0]
        o = json.loads(body)
        self.assertEqual((o["channel"], o["sequence"], o["version"], o["floor"]), ("stable", 2, 1, 1))
        self.assertEqual(signatures[0][0], sm.public_key(release.TEST_RECOVERY_SEED))
        k, _ = release.check_chain(release.destination(self.dest), now=NOW)
        with self.assertRaisesRegex(release.ReleaseError, "names 0 of the 1 recovery keys"):
            release.order(release.destination(self.dest), "stable", k, 1, 1, seeds=[release.TEST_RELEASE_SEED])
        with self.assertRaisesRegex(release.ReleaseError, "1 <= floor <= version"):
            release.order(release.destination(self.dest), "stable", k, 1, 2)

    def test_a_server_without_a_key_file(self):
        # The test key file goes up with a channel the test release key signs; with real keys none is made.
        release.publish(self.staged(1), self.dest, now=NOW)
        k, why = release.check_keys((self.dest / "keys/current").read_bytes(), now=NOW)
        self.assertIsNone(why)
        self.assertEqual(k["roles"]["release"], ([sm.public_key(release.TEST_RELEASE_SEED)], 1))
        other = self.dir / "other"
        other.mkdir()
        with self.assertRaisesRegex(release.ReleaseError, "keys/current: missing: publish a key file first"):
            release.publish(self.staged(2), other, now=NOW, seed=hashlib.sha256(b"a real release key").digest())
        self.assertFalse((other / "keys/current").exists())


if __name__ == "__main__":
    unittest.main(verbosity=1)
