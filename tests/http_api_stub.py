#!/usr/bin/env python3
"""A stand-in for the APIs MIND Core's clients talk to (000-NET-0047), over HTTPS with the suites' test certificates:
a JSON POST, a multipart upload, redirect chains, gzip and deflate bodies and a bomb, a stream of Server-Sent Events, a
chunked upload, Retry-After in both forms, and HEAD, PATCH and DELETE. `webcheck --api` checks each (tests/qemu_smoke.py, `api_runs`). For `get`
(000-NET-0048, `get_runs`): an echo of the request, a redirect to the same host and to another, and a slow answer.

The bodies the guest decodes and the events it reads are made here from fixed rules, so the suite knows what to expect.
"""
import email.parser
import email.policy
import gzip
import hashlib
import http.server
import json
import socketserver
import ssl
import threading
import time
import zlib

# What /api/gzip and /api/deflate send, decoded: the suite compares the guest's SHA-256 with this.
TEXT = b"".join(b"%06d the API's coded body, line after line\n" % i for i in range(5000))
TEXT_SHA256 = hashlib.sha256(TEXT).hexdigest()
EVENTS = 1000


class Api(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"
    port = 0  # the server's own, for the downgrade's Location

    def _send(self, status, body=b"", headers=()):
        self.send_response(status)
        for name, value in headers:
            self.send_header(name, value)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        if self.command != "HEAD":
            self.wfile.write(body)

    def _json(self, status, value):
        self._send(status, json.dumps(value, separators=(",", ":")).encode(), [("Content-Type", "application/json")])

    def _body(self):
        if self.headers.get("Transfer-Encoding", "").lower() == "chunked":
            data = b""
            while True:
                size = int(self.rfile.readline().split(b";")[0], 16)
                if size == 0:
                    while self.rfile.readline() not in (b"\r\n", b"\n", b""):
                        pass
                    return data
                data += self.rfile.read(size)
                self.rfile.readline()
        return self.rfile.read(int(self.headers.get("Content-Length", 0)))

    def _echo(self, data):
        # What arrived: the method, the headers `get` sets or must not carry on, and the body's length and SHA-256.
        self._json(200, {"method": self.command, "authorization": self.headers.get("Authorization"), "type": self.headers.get("Content-Type"),
                         "token": self.headers.get("X-Token"), "length": len(data), "sha256": hashlib.sha256(data).hexdigest()})

    def do_PUT(self):
        data = self._body()
        self._echo(data) if self.path == "/api/echo" else self._send(404)

    def do_PATCH(self):
        self.do_PUT()

    def do_HEAD(self):
        # GET's head alone: its Content-Length or chunked coding is named, no body follows (RFC 9110 9.3.2).
        self.do_GET()

    def do_DELETE(self):
        data = self._body()
        if self.path == "/api/echo":
            self._echo(data)
        elif self.path == "/api/item":
            self.send_response(204)
            self.end_headers()
        else:
            self._send(404)

    def do_POST(self):
        data = self._body()
        if self.path == "/api/echo":
            self._echo(data)
        elif self.path == "/api/chat":
            request = json.loads(data)
            self._json(200, {"model": request.get("model"), "content": request["messages"][0]["content"][::-1], "bytes": len(data)})
        elif self.path == "/api/upload":
            # The multipart body read by Python's own parser, not the guest's writer.
            message = email.parser.BytesParser(policy=email.policy.HTTP).parsebytes(
                b"Content-Type: " + self.headers["Content-Type"].encode() + b"\r\n\r\n" + data)
            parts = {}
            for part in message.iter_parts():
                payload = part.get_payload(decode=True)
                name = part.get_param("name", header="content-disposition")
                if part.get_filename():
                    parts[name] = {"filename": part.get_filename(), "type": part.get_content_type(), "length": len(payload), "sha256": hashlib.sha256(payload).hexdigest()}
                else:
                    parts[name] = payload.decode()
            self._json(200, {"parts": parts, "length": len(data)})
        elif self.path == "/api/stream":
            self._json(200, {"chunked": self.headers.get("Transfer-Encoding") == "chunked", "length": len(data), "sha256": hashlib.sha256(data).hexdigest()})
        elif self.path == "/api/see-other":
            self._send(303, b"", [("Location", "/api/final?from=post")])
        else:
            self._send(404)

    def do_GET(self):
        path, _, query = self.path.partition("?")
        if path.startswith("/api/r/"):
            # /api/r/N leads to /api/r/N-1 by 301, 302, 307 and 308 in turn, then to /api/final.
            left = int(path.rsplit("/", 1)[1])
            status = (301, 302, 307, 308)[left % 4]
            target = f"{left - 1}" if left > 1 else "/api/final"  # relative to /api/r/, then an absolute path
            self._send(status, b"moved", [("Location", target)])
        elif path == "/api/echo":
            self._echo(b"")
        elif path == "/api/hop":
            self._send(302, b"", [("Location", "/api/echo")])
        elif path == "/api/elsewhere":
            # Another host for the redirect rules: the stub at the port the query names.
            self._send(302, b"", [("Location", f"https://10.0.2.2:{int(query)}/api/echo")])
        elif path == "/api/slow":
            time.sleep(8)
            self._send(200, b"late")
        elif path == "/api/final":
            self._send(200, b"the end " + query.encode(), [("Content-Type", "text/plain")])
        elif path == "/api/loop":
            n = int(query or 0)
            self._send(302, b"", [("Location", f"/api/loop?{n + 1}")])
        elif path == "/api/down":
            self._send(301, b"", [("Location", f"http://10.0.2.2:{self.port}/api/final")])
        elif path == "/api/gzip":
            self._send(200, gzip.compress(TEXT, mtime=0), [("Content-Encoding", "gzip"), ("Content-Type", "text/plain")])
        elif path == "/api/deflate":
            self._send(200, zlib.compress(TEXT), [("Content-Encoding", "deflate"), ("Content-Type", "text/plain")])
        elif path == "/api/bomb":
            self._send(200, gzip.compress(bytes(64 << 20), compresslevel=9, mtime=0), [("Content-Encoding", "gzip")])
        elif path == "/api/events":
            # 1000 events in chunks of uneven sizes, with comments and CR LF line ends mixed in.
            self.send_response(200)
            self.send_header("Content-Type", "text/event-stream; charset=utf-8")
            self.send_header("Transfer-Encoding", "chunked")
            self.end_headers()
            if self.command == "HEAD":
                return
            stream = b"retry: 3000\n\n" + b"".join(
                (b": keep-alive\r\n" if i % 100 == 0 else b"") + b"event: delta\nid: %d\ndata: {\"n\":%d,\n%s" % (i, i, b"data: \"text\":\"piece %d\"}\r\n\r\n" % i)
                for i in range(EVENTS)) + b"event: done\ndata: [DONE]\n\n"
            at, size = 0, 1
            while at < len(stream):
                piece = stream[at:at + size]
                self.wfile.write(b"%x\r\n%s\r\n" % (len(piece), piece))
                at += len(piece)
                size = size * 7 % 1499 + 1
            self.wfile.write(b"0\r\n\r\n")
        elif path == "/api/busy":
            self._send(503, b"busy", [("Retry-After", "7")])
        elif path == "/api/limited":
            self._send(429, b"slow down", [("Retry-After", "Wed, 21 Oct 2015 07:28:00 GMT")])
        else:
            self._send(404)

    def log_message(self, *args):
        pass


def serve(certificate, key):
    """The stand-in on 127.0.0.1, its TLS from the certificate and key given; returns the server (port in server_address)."""
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    context.load_cert_chain(certificate, key)
    server = socketserver.ThreadingTCPServer(("127.0.0.1", 0), Api)
    server.daemon_threads = True
    server.socket = context.wrap_socket(server.socket, server_side=True)
    Api.port = server.server_address[1]
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server
