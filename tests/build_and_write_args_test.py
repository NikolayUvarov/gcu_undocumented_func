"""scripts/build_and_write.sh's argument checks: only the refusals of --mode= (751-LNK-0030), --machine, --old-disk and
--new-device-key, and --list-machines (801-LNK-0046), which end the script before anything is built or written. wslpath, which the script
calls before its arguments, is a stand-in."""
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
        # The pull before the options (801-LNK-0046) is not wanted here: this clone is never fetched by a test.
        self.env = {**os.environ, "PATH": f"{self.dir.name}:{os.environ['PATH']}", "TMPDIR": self.dir.name, "BUILD_AND_WRITE_UPDATED": "1"}

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


class Machines(unittest.TestCase):
    """--machine, --old-disk and --list-machines (801-LNK-0046): refused or answered before anything is fetched, built
    or written. HOME is a temporary directory holding dummy files only, and REPO is given, so git is not asked."""

    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.addCleanup(self.dir.cleanup)
        root = Path(self.dir.name)
        stub = root / "wslpath"
        stub.write_text("#!/bin/sh\nshift\necho \"$1\"\n")
        stub.chmod(0o755)
        self.home = root / "home"
        self.env = {**os.environ, "PATH": f"{root}:{os.environ['PATH']}", "TMPDIR": str(root), "HOME": str(self.home), "BUILD_AND_WRITE_UPDATED": "1",
                    "REPO": str(ROOT)}
        self.env.pop("MACHINES_KEEP", None)

    def run_script(self, *args):
        return subprocess.run(["bash", str(ROOT / "scripts/build_and_write.sh"), *args], capture_output=True, text=True,
                              env=self.env, timeout=60)

    def refused(self, *args):
        done = self.run_script(*args, "99")
        self.assertEqual(done.returncode, 1, done.stdout + done.stderr)
        return done.stderr

    def kept(self, machine, hostname, saved, parts, key="-"):
        d = self.home / ".config/mind/machines" / machine
        for part in parts:
            (d / part).mkdir(parents=True)
        (d / "machine.txt").write_text(f"id {machine}\nhostname {hostname}\nprint -\nsaved {saved}\ncommit -\ndevice-key {key}\n")
        return d

    def test_an_id_with_a_path_in_it(self):
        self.assertIn("--machine <id>: 1 to 68 of", self.refused("--keep-settings", "--machine", "../mind_00aa"))

    def test_an_old_disk_number_of_digits(self):
        self.assertIn("--old-disk <n>: the old disk's number", self.refused("--keep-settings", "--old-disk", "E:"))

    def test_machine_and_old_disk_need_a_keep_option(self):
        self.assertIn("go with --keep-settings, --keep-wifi or --keep-effector", self.refused("--machine", "mind_00aa"))
        self.assertIn("go with --keep-settings, --keep-wifi or --keep-effector", self.refused("--old-disk", "4"))

    def test_a_new_device_key_needs_keep_wifi(self):
        self.assertIn("--new-device-key goes with --keep-wifi or --keep-settings", self.refused("--keep-effector", "--new-device-key"))

    def test_list_machines(self):
        mac = self.kept("mind_00000000000000aa", "mind-mbp2", "2026-11-14T10:00Z", ["settings", "effector", "keystore"], "MIND 5097010F")
        (mac / "keystore/device.key").write_text("dummy-secret-key")
        (mac / "effector/token").write_text("dummy-secret-token")
        self.kept("host-mind-ryzen", "mind-ryzen", "2026-11-13T09:30Z", ["settings"])
        done = self.run_script("--list-machines")
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
        rows = [line.split() for line in done.stdout.splitlines()]
        self.assertEqual(rows[0], ["ID", "HOSTNAME", "SAVED", "KEPT", "DEVICE", "KEY"])
        self.assertEqual(rows[1:], [["host-mind-ryzen", "mind-ryzen", "2026-11-13T09:30Z", "settings", "-"],
                                    ["mind_00000000000000aa", "mind-mbp2", "2026-11-14T10:00Z", "settings,effector,keystore", "MIND", "5097010F"]])
        self.assertNotIn("dummy-secret", done.stdout + done.stderr)
        self.assertEqual(sorted(p.name for p in Path(self.dir.name).iterdir()), ["home", "wslpath"], "the script's copy removed")

    def test_list_machines_with_none(self):
        done = self.run_script("--list-machines")
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
        self.assertIn("No machine's settings are kept in", done.stdout)


class PullFirst(unittest.TestCase):
    """The pull comes before the options are read (801-LNK-0046): an old copy given an option only the new script knows
    pulls, and the new one runs with the same arguments. A clone of a temporary repository stands in for the
    maintainer's; nothing of this repository is fetched."""

    def git(self, *args, cwd):
        subprocess.run(["git", *args], cwd=cwd, check=True, capture_output=True, text=True,
                       env={**os.environ, "GIT_AUTHOR_NAME": "t", "GIT_AUTHOR_EMAIL": "t@t", "GIT_COMMITTER_NAME": "t",
                            "GIT_COMMITTER_EMAIL": "t@t"})

    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.addCleanup(self.dir.cleanup)
        root = Path(self.dir.name)
        stub = root / "wslpath"
        stub.write_text("#!/bin/sh\nshift\necho \"$1\"\n")
        stub.chmod(0o755)
        self.root = root
        self.env = {**os.environ, "PATH": f"{root}:{os.environ['PATH']}", "TMPDIR": str(root)}
        for name in ("BUILD_AND_WRITE_UPDATED", "BUILD_AND_WRITE_COPY", "BUILD_AND_WRITE_SELF", "REPO", "BRANCH"):
            self.env.pop(name, None)
        # The origin's fast-test holds this script; the maintainer's clone is made from it, then the origin moves on.
        origin, work, other = root / "origin.git", root / "work", root / "other"
        self.git("init", "-q", "--bare", "-b", "fast-test", str(origin), cwd=root)
        self.git("clone", "-q", str(origin), str(other), cwd=root)
        (other / "scripts").mkdir()
        (other / "scripts/build_and_write.sh").write_text((ROOT / "scripts/build_and_write.sh").read_text())
        self.git("add", "-A", cwd=other)
        self.git("commit", "-q", "-m", "this script", cwd=other)
        self.git("push", "-q", "origin", "HEAD:fast-test", cwd=other)
        self.git("clone", "-q", "-b", "fast-test", str(origin), str(work), cwd=root)
        (other / "scripts/build_and_write.sh").write_text('#!/usr/bin/env bash\necho "THE NEW SCRIPT RAN: $*"\n')
        self.git("commit", "-q", "-am", "a newer script", cwd=other)
        self.git("push", "-q", "origin", "HEAD:fast-test", cwd=other)
        self.work = work

    def test_an_option_the_old_copy_does_not_know(self):
        done = subprocess.run(["bash", str(self.work / "scripts/build_and_write.sh"), "--an-option-of-tomorrow", "99"],
                              capture_output=True, text=True, env=self.env, timeout=60)
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
        self.assertIn("build_and_write.sh changed with the pull: running the new one", done.stdout)
        self.assertIn("THE NEW SCRIPT RAN: --an-option-of-tomorrow 99", done.stdout)
        self.assertIn("echo", (self.work / "scripts/build_and_write.sh").read_text(), "the clone was pulled")

    def test_an_unknown_option_once_current(self):
        env = {**self.env, "BUILD_AND_WRITE_UPDATED": "1"}
        done = subprocess.run(["bash", str(ROOT / "scripts/build_and_write.sh"), "--no-such-option", "99"],
                              capture_output=True, text=True, env=env, timeout=60)
        self.assertEqual(done.returncode, 1, done.stdout + done.stderr)
        self.assertIn("unknown option --no-such-option", done.stderr)


if __name__ == "__main__":
    unittest.main()
