#!/usr/bin/env python3
"""351-ASR-0006: seeded fuzzing of the release checker's channel parser (scripts/release.py `check`). Channel files
signed with the test release key, as anyone may sign since that key is public, are mutated in their fields and bytes;
`check` must answer None or a reason for every one, and never raise (MC-9.4, MC-12.2). A run is evidence of the inputs
it made, not a proof. MIND_FUZZ_SEED and MIND_FUZZ_ITERATIONS override the fixed seed and count."""
import collections
import datetime
import json
import os
import random
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
sys.path.insert(0, str(ROOT / "tests"))
import release  # noqa: E402
import sign_manifest as sm  # noqa: E402
from release_test import NOW, volume  # noqa: E402

SEED = int(os.environ.get("MIND_FUZZ_SEED", "0x351A0006"), 0)
ITERATIONS = int(os.environ.get("MIND_FUZZ_ITERATIONS", "3000"))

# Values of every JSON type, and the edges of the ones the channel uses.
VALUES = [None, True, False, 0, 1, -1, 2**63, 1.5, float("inf"), "", "x", "stable", "1", [], [1], {}, {"x": 1},
          "2026-13-40T99:99:99Z", "2099-01-01T00:00:00Z", "2000-01-01T00:00:00Z", "2099-01-01", "a" * 64, "0" * 64]


def signed(body):
    return body + b"ed25519 " + sm.sign(release.release_seed(), body).hex().encode() + b"\n"


class ChannelFuzz(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.dir = Path(self.temp.name)
        self.dest = self.dir / "server"
        self.dest.mkdir()
        staged = release.stage(1, self.dir / "staged", {"x86_64": volume(self.dir / "volume", b"\x7fELF kernel")})
        release.publish(staged, self.dest, now=NOW)
        self.channel = self.dest / "channels" / "stable"
        self.valid = json.loads(self.channel.read_bytes().split(b"\n")[0])

    def tearDown(self):
        self.temp.cleanup()

    def mutated(self, rng):
        body = json.loads(json.dumps(self.valid))
        for _ in range(1 + rng.randrange(3)):
            key = rng.choice(list(body) + ["extra"])
            action = rng.randrange(5)
            if action == 0 and key in body: del body[key]
            elif action == 1: body[key] = rng.choice(VALUES)
            elif action == 2 and isinstance(body.get("manifests"), dict):
                body["manifests"][rng.choice(["x86_64", "aarch64", ""])] = rng.choice(VALUES)
            elif action == 3 and isinstance(body.get("version"), int): body["minimum"] = body["version"] + rng.choice([1, 100])
            else: body[key] = rng.choice(VALUES)
        if rng.random() < 0.8:
            text = json.dumps(body, sort_keys=True, separators=(",", ":")) + "\n"  # the one encoding, so the fields are reached
        else:
            text = json.dumps(body) + "\n"
        data = bytearray(text.encode())
        if rng.random() < 0.2 and len(data) > 2:
            for _ in range(1 + rng.randrange(3)):
                data[rng.randrange(len(data) - 1)] = rng.randrange(256)
        return bytes(data)

    def test_every_signed_channel_gets_an_answer(self):
        rng = random.Random(SEED)
        raised = collections.OrderedDict()
        for _ in range(ITERATIONS):
            body = self.mutated(rng)
            self.channel.write_bytes(signed(body) if rng.random() < 0.95 else body + b"ed25519 00\n")
            try:
                answer = release.check(self.dest, now=NOW)
            except Exception as e:  # noqa: BLE001 - every exception is the finding
                raised.setdefault(f"{type(e).__name__}: {e}"[:120], body)
                continue
            self.assertTrue(answer is None or isinstance(answer, str), answer)
        print(f"channel check: {ITERATIONS} signed channels (seed {SEED:#x}), {len(raised)} kinds of exception")
        for kind, body in raised.items():
            print(f"  {kind}\n    body {body[:160]!r}")
        self.assertEqual(list(raised), [], "release.check raised instead of answering")


if __name__ == "__main__":
    unittest.main()
