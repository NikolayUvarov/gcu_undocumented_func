"""scripts/build_and_write.sh's --mode= checks: only the refusals, which end the script before anything is built or
written (751-LNK-0030). wslpath, which the script calls before its arguments, is a stand-in."""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]


class ModeWords(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.addCleanup(self.dir.cleanup)
        stub = Path(self.dir.name) / "wslpath"
        stub.write_text("#!/bin/sh\nshift\necho \"$1\"\n")
        stub.chmod(0o755)
        self.env = {**os.environ, "PATH": f"{self.dir.name}:{os.environ['PATH']}", "TMPDIR": self.dir.name}

    def refused(self, *args):
        done = subprocess.run(["bash", str(ROOT / "scripts/build_and_write.sh"), *args, "99"], capture_output=True, text=True,
                              env=self.env, timeout=60)
        self.assertEqual(done.returncode, 1, done.stdout + done.stderr)
        return done.stderr

    def test_an_unknown_word(self):
        err = self.refused("--mode=development,turbo")
        self.assertIn("comma-separated: turbo", err)
        self.assertIn("remote-update, link-remote-update", err)

    def test_remote_update_needs_development_or_maintenance(self):
        self.assertIn("--mode=remote-update needs development or maintenance", self.refused("--mode=remote-update"))

    def test_link_remote_update_needs_link_console(self):
        self.assertIn("--mode=link-remote-update needs link-console", self.refused("--link-dev", "--mode=development,remote-update,link-remote-update"))

    def test_link_console_still_needs_link_dev(self):
        self.assertIn("--mode=link-console needs --link-dev", self.refused("--mode=development,link-console,link-remote-update"))

    def test_nothing_is_left_behind(self):
        self.refused("--mode=remote-update")
        self.assertEqual([p.name for p in Path(self.dir.name).iterdir()], ["wslpath"], "the script's copy removed")


if __name__ == "__main__":
    unittest.main()
