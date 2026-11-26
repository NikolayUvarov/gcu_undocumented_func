"""scripts/build_and_write.sh's argument checks: only the refusals of --mode= (751-LNK-0030), --machine, --old-disk and
--new-device-key, and --list-machines (801-LNK-0046), which end the script before anything is built or written. wslpath, which the script
calls before its arguments, is a stand-in. --board (100004-MOB-0011): its refusals, and its steps in a stand-in repository whose build,
packer, card maker and PowerShell only record what they were asked: no build, no disk, no Windows."""
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


class Board(unittest.TestCase):
    """--board <name> (100004-MOB-0011): refused with a name the catalog lacks and with the options of a PC's disk; its
    steps run in a stand-in repository (REPO): the aarch64 build, its image, the card, the writer given the card, link.conf."""

    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.addCleanup(self.dir.cleanup)
        root = Path(self.dir.name)
        self.bin = root / "bin"
        self.bin.mkdir()
        self.stub("wslpath", '#!/bin/sh\nshift\necho "$1"\n')
        self.env = {**os.environ, "PATH": f"{self.bin}:{os.environ['PATH']}", "TMPDIR": str(root), "HOME": str(root / "home"),
                    "BUILD_AND_WRITE_UPDATED": "1", "REPO": str(ROOT)}
        self.root = root

    def stub(self, name, text, where=None):
        path = (where or self.bin) / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)
        path.chmod(0o755)
        return path

    def run_script(self, *args):
        return subprocess.run(["bash", str(ROOT / "scripts/build_and_write.sh"), *args], capture_output=True, text=True,
                              env=self.env, timeout=120)

    def refused(self, *args):
        done = self.run_script(*args, "99")
        self.assertEqual(done.returncode, 1, done.stdout + done.stderr)
        return done.stderr

    def test_a_board_the_catalog_lacks(self):
        err = self.refused("--board", "pinephone")
        self.assertIn("--board <name>: one of orangepi-zero-plus2, orangepi-zero3", err)
        self.assertIn("--board <name>: a name of scripts/boards.toml", self.refused("--board", "../x"))

    def test_the_options_of_a_pcs_disk(self):
        for option in (["--keep-settings"], ["--keep-wifi"], ["--keep-effector", "--machine", "mind_00aa"], ["--wifitest"], ["--wifi-ofdm=6"], ["--wifijoin"]):
            with self.subTest(option=option):
                self.assertIn("--board makes a board's new card:", self.refused("--board", "orangepi-zero3", *option))

    def test_a_link_rate_other_than_the_firmwares(self):
        self.assertIn("a board's console UART keeps the firmware's 115200", self.refused("--board", "orangepi-zero3", "--link=460800"))

    def repository(self):
        """A stand-in repository: each step appends what it was asked to steps.log and writes what the next one reads."""
        repo = self.root / "repo"
        log = self.root / "steps.log"
        self.stub("02_build.sh", f'#!/bin/bash\necho "build ARCH=${{ARCH:-x86_64}} $*" >> {log}\nmkdir -p aarch64_root usb_root\n'
                  'echo "commit 0123" > aarch64_root/MANIFEST\necho "commit 4567" > usb_root/MANIFEST\n', repo)
        self.stub("scripts/make_usb_image.py", f'#!/usr/bin/env python3\nimport sys, pathlib\nopen("{log}", "a").write("pack " + " ".join(sys.argv[1:]) + "\\n")\n'
                  'out = pathlib.Path("dist/mind-core-usb-aarch64.img" if "aarch64" in sys.argv else "dist/mind-core-usb.img")\n'
                  'out.parent.mkdir(exist_ok=True); out.write_bytes(b"image")\n', repo)
        self.stub("scripts/board_image.py", f'#!/usr/bin/env python3\nimport sys, pathlib\nif sys.argv[1:] == ["--list"]:\n'
                  '    print("orangepi-zero-plus2\\tH5\\norangepi-zero3\\tH618"); sys.exit(0)\n'
                  f'open("{log}", "a").write("card " + " ".join(sys.argv[1:]) + "\\n")\n'
                  'out = pathlib.Path(sys.argv[sys.argv.index("--output") + 1]); out.parent.mkdir(parents=True, exist_ok=True); out.write_bytes(b"card")\n', repo)
        self.stub("scripts/proprietary.sh", f'#!/bin/sh\necho "proprietary $*" >> {log}\n', repo)
        self.stub("scripts/disk_copy.sh", f'#!/bin/sh\necho "disk_copy $*" >> {log}\n', repo)
        (repo / "05_write_usb_windows.ps1").write_text("")
        # PowerShell: the code it is given, decoded, in powershell.log; link.conf's write answered as the real one answers.
        self.stub("powershell.exe", '#!/usr/bin/env python3\nimport base64, sys\ncode = base64.b64decode(sys.argv[sys.argv.index("-EncodedCommand") + 1]).decode("utf-16-le")\n'
                  f'open("{self.root / "powershell.log"}", "a").write(code + "\\n----\\n")\n'
                  'if "link.conf" in code:\n    conf = code.split("-Value \'")[1].split("\'")[0]\n    print(f"link: E:\\\\link.conf written ({conf}), read back")\n')
        self.env["REPO"] = str(repo)
        return repo, log

    def test_the_steps_of_a_boards_card(self):
        repo, log = self.repository()
        done = self.run_script("--board", "orangepi-zero3", "--link-dev", "--mode=development", "7")
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
        steps = log.read_text().splitlines()
        self.assertEqual(steps, ["build ARCH=aarch64 ",
                                 "pack --arch aarch64 --no-build --force",
                                 f"card --board orangepi-zero3 --image {repo}/dist/mind-core-usb-aarch64.img --output {repo}/dist/boards/mind-orangepi-zero3.img --force"],
                         "no proprietary files, pictures or the Mac's script on a board's card")
        shell = (self.root / "powershell.log").read_text()
        self.assertIn(f"'-File','\"{repo}/05_write_usb_windows.ps1\"','-DiskNumber','7','-Image','\"{repo}/dist/boards/mind-orangepi-zero3.img\"'", shell)
        self.assertIn("link: E:\\link.conf written (baud=115200 run dev uart), read back", done.stdout)
        self.assertIn("mode.conf", shell)
        self.assertIn("The card of orangepi-zero3 from", done.stdout)

    def test_a_boards_card_from_a_given_image_with_slots_refused(self):
        repo, log = self.repository()
        given = self.root / "given" / "mind-core-usb-aarch64.img"
        given.parent.mkdir()
        given.write_bytes(b"image")
        (given.parent / "MANIFEST").write_text("commit 89ab\n")
        done = self.run_script("--board", "orangepi-zero-plus2", "--image", str(given), "3")
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
        self.assertEqual(log.read_text().splitlines(),
                         [f"card --board orangepi-zero-plus2 --image {given} --output {repo}/dist/boards/mind-orangepi-zero-plus2.img --force"])
        self.assertIn("The card made from the given image", done.stdout)
        self.assertNotIn("link.conf", (self.root / "powershell.log").read_text(), "no link.conf without --link: the shell keeps the console")
        self.assertIn("--slots builds the image; it does not go with --image", self.refused("--board", "orangepi-zero3", "--slots", "--image", str(given)))

    def test_a_pcs_disk_is_as_before(self):
        repo, log = self.repository()
        done = self.run_script("--link", "5")
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
        self.assertEqual(log.read_text().splitlines()[:3], ["build ARCH=x86_64 ", "pack --no-build --force", "proprietary fetch"])
        self.assertIn("link: E:\\link.conf written (baud=460800), read back", done.stdout)


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
