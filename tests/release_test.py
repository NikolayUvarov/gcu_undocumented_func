#!/usr/bin/env python3
"""Tests of releases (scripts/release.py, issue 351-UPD-0005; MC-9.2, 9.4, 9.6): a staged release published to a
directory verifies; a version not above the channel's is refused; an upload cut before the channel leaves the old
channel whole; a changed channel, manifest or blob, an expired channel and a channel signed with the boot key are
refused by the checker; a signed body that is not a channel gets a reason, never an exception (351-UPD-0014)."""
import datetime
import hashlib
import json
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
import release  # noqa: E402
import sign_manifest as sm  # noqa: E402

NOW = datetime.datetime(2026, 10, 8, tzinfo=datetime.timezone.utc)


def volume(where, kernel):
    v = Path(where)
    (v / "EFI/BOOT").mkdir(parents=True)
    (v / "EFI/BOOT/BOOTX64.EFI").write_bytes(b"MZ loader")
    (v / "kernel.elf").write_bytes(kernel)
    (v / "shell.elf").write_bytes(b"\x7fELF the same in every release")
    sm.sign_volume(v, sm.TEST_SEED, ("c" * 40, "nightly-x", "d" * 64))
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
        v = volume(self.dir / f"volume{version}", kernel or f"\x7fELF kernel {version}".encode())
        return release.stage(version, self.dir / f"staged{version}", {"x86_64": v})

    def check(self, **kw):
        return release.check(self.dest, now=NOW, **kw)

    def test_a_published_release_verifies(self):
        body = release.publish(self.staged(1), self.dest, now=NOW)
        self.assertIsNone(self.check())
        c = json.loads(body)
        self.assertEqual((c["version"], c["minimum"], c["expires"]), (1, 1, "2026-11-07T00:00:00Z"))
        # Files are stored once, by content: a second release adds only what changed.
        release.publish(self.staged(2), self.dest, minimum=1, now=NOW)
        self.assertIsNone(self.check())
        self.assertEqual(len(list((self.dest / "blobs").iterdir())), 4)

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
        self.assertEqual(self.check(), "channels/stable: bad signature")
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
            channel.write_bytes(body + b"ed25519 " + sm.sign(release.release_seed(), body).hex().encode() + b"\n")
            return self.check()
        cases = [
            (b"\xff\xfe not UTF-8\n", "not UTF-8 JSON"),
            (b"{not json\n", "not UTF-8 JSON"),
            (b"[" * 100000 + b"\n", "not UTF-8 JSON"),
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
        # A channel signed with the boot key is refused: the keys have different purposes (MC-9.6).
        release.publish(self.staged(1), self.dest, now=NOW, seed=sm.TEST_SEED)
        self.assertEqual(self.check(), "channels/stable: bad signature")
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

    def test_the_built_volumes(self):
        # The volumes the build signed, if they are built: staged, published and verified as a release.
        volumes = {a: ROOT / v for a, v in release.ARCHES.items() if (ROOT / v / "MANIFEST").exists()}
        if not volumes:
            self.skipTest("no built volume")
        release.publish(release.stage(1, self.dir / "built", volumes), self.dest, now=NOW)
        self.assertIsNone(self.check())


if __name__ == "__main__":
    unittest.main(verbosity=1)
