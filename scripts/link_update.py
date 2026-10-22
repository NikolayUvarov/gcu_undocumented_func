#!/usr/bin/env python3
"""An update of MIND Core through the link (351-KRN-0141): this build as a signed release, put into data/link/rel/ on
the machine at the other end of the cable with mindlink, and the updater's log followed through the trial restart
until the new slot is confirmed or the update refused.

Usage:
    link_update.py PORT [--version N] [--arch x86_64|aarch64] [--root DIR] [--mindlink PATH] [--timeout S]
                        [--mindlink-option OPTION]...

    PORT          the serial port mindlink speaks on (COM10, /dev/ttyUSB0, tcp:HOST:PORT)
    --version N   the release's version (default: the time as YYYYMMDDHHMM, above any earlier one of this script)
    --root DIR    the signed build (default: usb_root, aarch64_root with --arch aarch64)
    --mindlink    the mindlink program (default: mindlink on the PATH)
    --timeout S   how long to follow the log after the channel is put (default 1800 s)
    --mindlink-option OPTION   passed to every mindlink call, once per word (--mindlink-option=--baud
                  --mindlink-option=460800)

The machine needs an image with slots and this update.txt at its root (make_usb_image.py --slots --update FILE),
and link.conf:
    source data/link/rel
    channel stable
    automatic apply
    every 20

The release is signed as scripts/release.py signs one: the manifest with the build's boot key, the channel with
$MIND_RELEASE_KEY, or the public test release key without it. Only what the updater reads goes over the cable: the
boot files' blobs (mindlink skips a file the other end holds already), the manifest and its signature, then the
channel, last. Exit status: 0 when the updater confirmed the new slot on the disk, 1 when a step was refused or the
time ran out, 2 for a usage error.
"""
import argparse
import datetime
import re
import subprocess
import sys
import tempfile
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import boot_slots  # noqa: E402
import release  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]
DIR = "rel"  # below data/link/: update.txt's `source data/link/rel`
DONE = re.compile(r"\[UPDATER\] SLOT ([AB]) CONFIRMED ON THE DISK")
FAILED = re.compile(r"\[UPDATER\] [^\n]*(REFUSED|ROLLBACK)[^\n]*")
ANCHOR = 8  # lines that mark where the last read of the log ended


def release_files(manifest, arch, version):
    """What the updater reads of a release, in the order it is put: the boot files' blobs, then the manifest and its
    signature. The channel, `channels/stable`, goes last and is not in the list."""
    boot = [digest for name, _, digest in release.manifest_files(manifest) if name == "kernel.elf" or name in boot_slots.BOOT_FILES]
    return [f"blobs/{digest}" for digest in boot] + [f"releases/{version}/{arch}/MANIFEST", f"releases/{version}/{arch}/MANIFEST.SIG"]


def build(root, arch, version, temp):
    """The signed build at `root` staged and published as release `version` into a server tree under `temp`."""
    staged, server = Path(temp) / "staged", Path(temp) / "server"
    release.stage(version, staged, {arch: root})
    release.publish(staged, server)
    return server, (Path(root) / "MANIFEST").read_bytes()


class Link:
    """mindlink calls to one port."""
    def __init__(self, mindlink, port, options=()):
        self.command = [str(mindlink), *options, port]

    def run(self, *args, ok=True, timeout=600):
        r = subprocess.run([*self.command, *args], capture_output=True, text=True, timeout=timeout)
        if ok and r.returncode != 0:
            raise RuntimeError(f"mindlink {' '.join(args)}: exit {r.returncode}: {(r.stdout + r.stderr).strip()[-500:]}")
        return r

    def put(self, path, name):
        self.run("mkdir", f"{DIR}/{name.rpartition('/')[0]}")
        self.run("put", str(path), f"{DIR}/{name}")

    def log(self):
        """The other end's log, or None while it does not answer (a restart)."""
        try:
            r = self.run("log", ok=False, timeout=120)
        except subprocess.TimeoutExpired:
            return None
        return r.stdout if r.returncode == 0 else None


def fresh(lines, anchor):
    """The lines after `anchor`, the last lines read before; all of them when the anchor is gone (the next boot)."""
    n = len(anchor)
    for i in range(len(lines) - n, -1, -1) if n else ():
        if lines[i:i + n] == anchor:
            return lines[i + n:]
    return lines


def follow(link, timeout, say=print, pause=5, before=""):
    """The updater's lines after `before` (the log as it was), across the trial restart, until the new slot is
    confirmed on the disk (its line returned) or a step is refused (an error)."""
    deadline, anchor, trial = time.monotonic() + timeout, before.splitlines()[-ANCHOR:], False
    while time.monotonic() < deadline:
        text = link.log()
        if text is None:
            time.sleep(pause)
            continue
        lines = text.splitlines()
        for line in fresh(lines, anchor):
            if "[UPDATER]" in line or "TRIAL" in line:
                say(line)
            if DONE.search(line):
                return line
            trial = trial or "ON TRIAL, VERSION" in line
            # A check that ran before the channel was whole finds it missing; once the new slot runs on trial, a check
            # finds nothing newer. Neither refusal is the update's.
            if FAILED.search(line) and "REFUSED: Missing" not in line and not (trial and "REFUSED: Older" in line):
                raise RuntimeError(line.strip())
        anchor = lines[-ANCHOR:] or anchor
        time.sleep(pause)
    raise RuntimeError(f"no confirmation within {timeout} s")


def update(link, server, manifest, arch, version, timeout, say=print):
    """Puts release `version` from `server` and follows the log to its end."""
    files = release_files(manifest, arch, version)
    started = time.monotonic()
    for index, name in enumerate(files, 1):
        link.put(Path(server) / name, name)
        say(f"PUT {index} OF {len(files)}: {name}")
    say(f"PUT {len(files)} FILES IN {time.monotonic() - started:.0f} S; NOW THE CHANNEL")
    # Before the channel the log's end is noted: the updater's lines that follow are this release's.
    before = link.log()
    if before is None:
        raise RuntimeError("the other end's log could not be read")
    link.put(Path(server) / "channels/stable", "channels/stable")
    return follow(link, timeout, say, before=before)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("port")
    parser.add_argument("--version", type=int, default=int(datetime.datetime.now(datetime.timezone.utc).strftime("%Y%m%d%H%M")))
    parser.add_argument("--arch", choices=sorted(release.ARCHES), default="x86_64")
    parser.add_argument("--root", type=Path)
    parser.add_argument("--mindlink", default="mindlink")
    parser.add_argument("--timeout", type=int, default=1800)
    parser.add_argument("--mindlink-option", action="append", default=[])
    args = parser.parse_args(argv)
    root = args.root or ROOT / release.ARCHES[args.arch]
    link = Link(args.mindlink, args.port, args.mindlink_option)
    try:
        print(link.run("hello").stdout.strip())
        with tempfile.TemporaryDirectory(prefix="mind-link-update-") as temp:
            server, manifest = build(root, args.arch, args.version, temp)
            print(f"RELEASE {args.version} OF {root} ({args.arch})")
            line = update(link, server, manifest, args.arch, args.version, args.timeout)
        print(f"CONFIRMED: {line.strip()}")
        return 0
    except (RuntimeError, release.ReleaseError, subprocess.TimeoutExpired) as e:
        print(f"link_update: {e}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
