#!/usr/bin/env python3
"""Serves a release directory over HTTPS for tests (351-UPD-0005, docs/update/publishing.md): the layout of
scripts/release.py, read-only, with a certificate the caller gives (the TLS suite's test CA, or any). Not a production
server: a real one is any static web server over the same directory.

Usage: serve_release.py DIR --cert CERT.pem --key KEY.pem [--port 8443] [--host 127.0.0.1]
"""
import functools
import http.server
import ssl
import sys
import threading


def serve(directory, cert, key, host="127.0.0.1", port=0):
    """Starts the server in a thread; returns it (its port is server.server_address[1])."""
    handler = functools.partial(http.server.SimpleHTTPRequestHandler, directory=str(directory))
    handler.log_message = lambda *args: None
    server = http.server.ThreadingHTTPServer((host, port), handler)
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    context.minimum_version = ssl.TLSVersion.TLSv1_3
    context.load_cert_chain(cert, key)
    server.socket = context.wrap_socket(server.socket, server_side=True)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server


def main(argv):
    if len(argv) < 5 or "--cert" not in argv or "--key" not in argv:
        print(__doc__)
        return 2
    option = lambda name, default=None: argv[argv.index(name) + 1] if name in argv else default
    server = serve(argv[0], option("--cert"), option("--key"), option("--host", "127.0.0.1"), int(option("--port", "8443")))
    print(f"SERVING {argv[0]} ON https://{server.server_address[0]}:{server.server_address[1]}/", flush=True)
    threading.Event().wait()
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
