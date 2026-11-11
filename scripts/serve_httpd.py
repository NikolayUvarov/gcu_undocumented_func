#!/usr/bin/env python3
"""Runs the project's release server, tools/httpd, for the QEMU checks (110-NET-0068): built with its test hooks, over
a release directory, with the interface of serve_release.serve's server: `server_address`, `cuts` (path to a byte count
the next response for it is cut after), `holds` (path to an event the next response for it waits for, at most 10 s),
`requests` ((path, Range header) per request) and `shutdown()`.

Usage: serve_httpd.py DIR [--cert CERT.pem --key KEY.pem] (prints the server's address, then serves until stopped)
"""
import subprocess
import sys
import tempfile
import threading
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
_built = {}


def build():
    """httpd with its test hooks, built once in this machine's temporary directory; returns the program's path."""
    if "httpd" not in _built:
        target = Path(tempfile.gettempdir()) / "mind-httpd-target"
        subprocess.run(["cargo", "build", "--release", "--quiet", "--features", "test-hooks", "--manifest-path",
                        str(ROOT / "tools/httpd/Cargo.toml"), "--target-dir", str(target)], check=True)
        _built["httpd"] = target / "release/httpd"
    return _built["httpd"]


def _name(path):
    # A path as httpd's hooks name it: without its first `/`, each `/` as `+`.
    return path.lstrip("/").replace("/", "+")


class _Cuts:
    def __init__(self, hooks):
        self.hooks = hooks

    def __setitem__(self, path, count):
        new = self.hooks / f".cut-{_name(path)}"
        new.write_text(f"{count}\n")
        new.rename(self.hooks / "cut" / _name(path))  # whole, as the server reads it


class _Holds:
    def __init__(self, hooks):
        self.hooks = hooks

    def __setitem__(self, path, event):
        (self.hooks / "hold" / _name(path)).write_text("")

        def release():
            event.wait()
            for where in ("hold", "held"):  # not taken yet, or taken and waiting
                (self.hooks / where / _name(path)).unlink(missing_ok=True)
        threading.Thread(target=release, daemon=True).start()


class Server:
    """httpd over `directory` on 127.0.0.1, its port chosen by the system; over TLS 1.3 with `cert` and `key`."""

    def __init__(self, directory, cert=None, key=None, timeout=60):
        self.base = Path(tempfile.mkdtemp(prefix="mind-httpd-"))
        self.hooks, self.logs = self.base / "hooks", self.base / "logs"
        self.logs.mkdir()
        # The emulated machines of a loaded host can be slow: the deadlines are `timeout` s, not the 10 s a server keeps.
        command = [str(build()), str(directory), "--listen", "127.0.0.1:0", "--test-hooks", str(self.hooks), "--log", str(self.logs),
                   "--head-timeout", str(timeout), "--idle-timeout", str(timeout), "--send-timeout", str(timeout)]
        if cert:
            command += ["--cert", str(cert), "--key", str(key)]
        self.output = self.base / "httpd.out"
        with open(self.output, "w") as out:
            self.process = subprocess.Popen(command, stdout=out, stderr=subprocess.STDOUT)
        deadline = time.monotonic() + 30
        while "HTTPD: SERVING " not in self.output.read_text():
            if self.process.poll() is not None or time.monotonic() > deadline:
                self.process.kill()
                raise AssertionError(("httpd did not start", self.output.read_text()))
            time.sleep(.05)
        address = self.output.read_text().split("HTTPD: SERVING ", 1)[1].split("\n", 1)[0].rsplit(" ON ", 1)[1]
        host, port = address.rsplit(":", 1)
        self.server_address = (host, int(port))
        self.cuts, self.holds = _Cuts(self.hooks), _Holds(self.hooks)

    @property
    def requests(self):
        try:
            lines = (self.hooks / "requests").read_text().splitlines()
        except FileNotFoundError:
            return []
        return [(path, None if asked == "-" else asked) for path, _, asked in (line.partition(" ") for line in lines)]

    def log(self):
        """Its log: one line a request (docs/update/server.md)."""
        return "".join(p.read_text() for p in sorted(self.logs.iterdir()))

    def shutdown(self):
        self.process.terminate()
        try:
            self.process.wait(10)
        except subprocess.TimeoutExpired:
            self.process.kill()


def serve(directory, cert=None, key=None):
    """Starts httpd over `directory`, over HTTPS with `cert` and `key` or plain HTTP without; returns it."""
    return Server(directory, cert, key)


def main(argv):
    if not argv or argv[0].startswith("--") or ("--cert" in argv) != ("--key" in argv):
        print(__doc__)
        return 2
    option = lambda name: argv[argv.index(name) + 1] if name in argv else None
    server = serve(argv[0], option("--cert"), option("--key"))
    print(f"SERVING {argv[0]} ON {'https' if option('--cert') else 'http'}://{server.server_address[0]}:{server.server_address[1]}/", flush=True)
    try:
        server.process.wait()
    except KeyboardInterrupt:
        server.shutdown()
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
