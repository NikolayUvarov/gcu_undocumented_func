#!/usr/bin/env python3
"""scripts/mirror_showcase.sh (000-KRN-0088) on local repositories: the first copy, nothing done without a change, a
change that waits for 48 hours and then goes, and a commit made in the showcase that stops the copy rather than being
overwritten."""
import datetime
import os
import subprocess
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "scripts/mirror_showcase.sh"
ENV = {**os.environ, "GIT_AUTHOR_NAME": "t", "GIT_AUTHOR_EMAIL": "t@t", "GIT_COMMITTER_NAME": "t", "GIT_COMMITTER_EMAIL": "t@t"}


def git(*args, cwd):
    return subprocess.run(["git", *args], cwd=cwd, env=ENV, check=True, capture_output=True, text=True).stdout.strip()


class Mirror(unittest.TestCase):
    def setUp(self):
        self.dir = Path(tempfile.mkdtemp(prefix="mirror-test-"))
        self.source, self.showcase, self.clone, self.work = (self.dir / n for n in ("source.git", "showcase.git", "clone", "work"))
        git("init", "-q", "--bare", "-b", "main", str(self.source), cwd=self.dir)
        git("init", "-q", "--bare", "-b", "main", str(self.showcase), cwd=self.dir)
        git("clone", "-q", str(self.source), str(self.work), cwd=self.dir)
        self.commit("first")
        git("tag", "v1", cwd=self.work)
        git("push", "-q", "origin", "HEAD:main", "--tags", cwd=self.work)
        git("clone", "-q", str(self.source), str(self.clone), cwd=self.dir)

    def tearDown(self):
        subprocess.run(["rm", "-rf", str(self.dir)], check=True)

    def commit(self, text, cwd=None):
        cwd = cwd or self.work
        (cwd / "file.txt").write_text(text)
        git("add", "file.txt", cwd=cwd)
        git("commit", "-q", "-m", text, cwd=cwd)

    def mirror(self, *args, last_push=None):
        env = {**ENV, "MIRROR_REPO": str(self.clone)}
        if last_push is not None:
            env["MIRROR_LAST_PUSH"] = (datetime.datetime.now(datetime.timezone.utc) - datetime.timedelta(hours=last_push)).isoformat()
        return subprocess.run(["bash", str(SCRIPT), *args, str(self.showcase)], env=env, capture_output=True, text=True)

    def shown(self):
        return git("rev-parse", "main", cwd=self.showcase)

    def test_copies_waits_and_stops(self):
        out = self.mirror()
        self.assertEqual(out.returncode, 0, out.stderr)
        self.assertEqual(self.shown(), git("rev-parse", "main", cwd=self.source))
        self.assertEqual(git("tag", cwd=self.showcase), "v1")
        # Nothing changed: nothing pushed.
        self.assertIn("nothing changed", self.mirror("--every-hours", "48", last_push=100).stdout)
        # A change waits while the last copy is under 48 hours old, then goes.
        self.commit("second")
        git("push", "-q", "origin", "HEAD:main", cwd=self.work)
        before = self.shown()
        out = self.mirror("--every-hours", "48", last_push=1)
        self.assertIn("changes wait", out.stdout)
        self.assertEqual(self.shown(), before)
        out = self.mirror("--every-hours", "48", last_push=49)
        self.assertEqual(out.returncode, 0, out.stderr)
        self.assertEqual(self.shown(), git("rev-parse", "main", cwd=self.source))
        # A commit made in the showcase stops the copy; it is not overwritten.
        other = self.dir / "other"
        git("clone", "-q", str(self.showcase), str(other), cwd=self.dir)
        self.commit("made in the showcase", cwd=other)
        git("push", "-q", "origin", "HEAD:main", cwd=other)
        self.commit("third")
        git("push", "-q", "origin", "HEAD:main", cwd=self.work)
        out = self.mirror()
        self.assertNotEqual(out.returncode, 0, out.stdout)
        self.assertEqual(git("log", "-1", "--format=%s", "main", cwd=self.showcase), "made in the showcase")
        print("PASS: mirror_showcase.sh: the first copy with its tag; nothing pushed without a change; a change waits under "
              "48 hours and goes after; a commit made in the showcase stops the copy and stays", flush=True)


if __name__ == "__main__":
    unittest.main(verbosity=1)
