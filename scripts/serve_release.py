#!/usr/bin/env python3
"""Serves a release directory for tests (351-UPD-0005, docs/update/publishing.md): the layout of scripts/release.py,
read-only, over HTTPS with a certificate the caller gives (the TLS suite's test CA, or any), or over plain HTTP. Single
byte ranges (`Range: bytes=N-` or `N-M`) are answered with 206, so downloads resume (351-NET-0001). Not a production
server: a real one is any static web server over the same directory.

Usage: serve_release.py DIR (--cert CERT.pem --key KEY.pem | --plain) [--port 8443] [--host 127.0.0.1]
"""
import functools
import http.server
import os
import re
import ssl
import sys
import threading


class RangeHandler(http.server.SimpleHTTPRequestHandler):
    """Static files with single byte ranges. A server's subclass holds `cuts`, path to a byte count (a test hook: the
    next response for that path is cut after that many body bytes), `raw`, path to the bytes sent as the whole response
    (a test hook: a malformed head), and `requests`, (path, Range header) per GET."""
    cuts = {}
    raw = {}
    requests = []

    def log_message(self, *args):
        pass

    def send_head(self):
        self.remaining = None
        self.requests.append((self.path, self.headers.get("Range")))
        if self.path in self.raw:
            self.wfile.write(self.raw[self.path])
            self.close_connection = True
            return None
        path = self.translate_path(self.path)
        wanted = re.fullmatch(r"bytes=(\d+)-(\d*)", (self.headers.get("Range") or "").strip())
        if not wanted or not os.path.isfile(path):
            return super().send_head()
        size = os.path.getsize(path)
        start, end = int(wanted[1]), int(wanted[2]) if wanted[2] else size - 1
        if start >= size or end < start:
            self.send_response(416)
            self.send_header("Content-Range", f"bytes */{size}")
            self.send_header("Content-Length", "0")
            self.end_headers()
            return None
        end = min(end, size - 1)
        source = open(path, "rb")
        source.seek(start)
        self.send_response(206)
        self.send_header("Content-Type", self.guess_type(path))
        self.send_header("Content-Range", f"bytes {start}-{end}/{size}")
        self.send_header("Content-Length", str(end - start + 1))
        self.end_headers()
        self.remaining = end - start + 1
        return source

    def copyfile(self, source, outputfile):
        limit = self.remaining
        cut = self.cuts.pop(self.path, None)
        if cut is not None:
            limit = cut if limit is None else min(limit, cut)
            self.close_connection = True
        while limit is None or limit > 0:
            chunk = source.read(65536 if limit is None else min(65536, limit))
            if not chunk:
                break
            outputfile.write(chunk)
            if limit is not None:
                limit -= len(chunk)


def serve(directory, cert=None, key=None, host="127.0.0.1", port=0, cuts=None, raw=None):
    """Starts the server in a thread, over HTTPS with `cert` and `key` or plain HTTP without; returns it (its port is
    server.server_address[1]; server.cuts and server.requests are its handler's)."""
    handler = type("Handler", (RangeHandler,), {"cuts": dict(cuts or {}), "raw": dict(raw or {}), "requests": []})
    server = http.server.ThreadingHTTPServer((host, port), functools.partial(handler, directory=str(directory)))
    server.daemon_threads = True
    server.cuts, server.requests = handler.cuts, handler.requests
    if cert:
        context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        context.minimum_version = ssl.TLSVersion.TLSv1_3
        context.load_cert_chain(cert, key)
        server.socket = context.wrap_socket(server.socket, server_side=True)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server


def main(argv):
    plain = "--plain" in argv
    if not argv or argv[0].startswith("--") or (not plain and ("--cert" not in argv or "--key" not in argv)):
        print(__doc__)
        return 2
    option = lambda name, default=None: argv[argv.index(name) + 1] if name in argv else default
    server = serve(argv[0], None if plain else option("--cert"), None if plain else option("--key"), option("--host", "127.0.0.1"), int(option("--port", "8443")))
    print(f"SERVING {argv[0]} ON {'http' if plain else 'https'}://{server.server_address[0]}:{server.server_address[1]}/", flush=True)
    threading.Event().wait()
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
