#!/usr/bin/env python3
"""Bounded HTTPS test double for docs/effector/protocol.md, contract 1.0."""

import argparse
import collections
import contextlib
import copy
import dataclasses
import hashlib
import hmac
import http.server
import json
import pathlib
import re
import ssl
import subprocess
import tempfile
import threading
import time
import urllib.parse


CONTRACT = "effector-agent 1.0"
JSON_LIMIT = 64 * 1024
ACK_LIMIT = 4 * 1024 * 1024  # a result with an artifact inline (a 2 MiB PNG as base64); the server takes 12 MiB
FILE_LIMIT = 1024 * 1024
MAX_SAMPLES = 64
MAX_COMMANDS = 128
MAX_AGENTS = 8
MAX_FILES = 16
MAX_REQUESTS = 4096
FINAL_STATES = {"completed", "failed", "timeout", "expired", "cancelled", "rollback",
                "run_as_unavailable", "interactive_session_unavailable", "denied"}
IDENTIFIER = re.compile(r"[A-Za-z0-9_-]{1,128}\Z")


def encoded(value):
    return json.dumps(value, ensure_ascii=True, separators=(",", ":"), allow_nan=False).encode()


def decoded(data):
    def object_pairs(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise ValueError("duplicate JSON key")
            result[key] = value
        return result

    def bad_constant(_):
        raise ValueError("non-finite JSON number")

    result = json.loads(data.decode("utf-8"), object_pairs_hook=object_pairs, parse_constant=bad_constant)
    if not isinstance(result, dict):
        raise ValueError("JSON object required")
    return result


def identifier(value):
    return isinstance(value, str) and IDENTIFIER.fullmatch(value) is not None


@dataclasses.dataclass
class Fault:
    kind: str
    value: object = None


class _Server(http.server.ThreadingHTTPServer):
    daemon_threads = True
    allow_reuse_address = True

    def __init__(self, address, fixture, tls_context):
        self.fixture = fixture
        self.tls_context = tls_context
        self.slots = threading.BoundedSemaphore(16)
        super().__init__(address, _Handler)

    def process_request(self, request, address):
        if not self.slots.acquire(blocking=False):
            self.shutdown_request(request)
            return
        try:
            super().process_request(request, address)
        except BaseException:
            self.slots.release()
            raise

    def process_request_thread(self, request, address):
        try:
            request.settimeout(5)
            with self.tls_context.wrap_socket(request, server_side=True) as secure:
                super().process_request_thread(secure, address)
        except (OSError, ssl.SSLError):
            request.close()
        finally:
            self.slots.release()

    def handle_error(self, request, address):
        self.fixture.errors.append("request handler failed")


class _Handler(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *_):
        pass

    def _reply(self, status, body=None, headers=None, raw=None):
        data = raw if raw is not None else (encoded(body) if body is not None else b"")
        self.record["status"] = status
        self.send_response(status)
        self.send_header("Content-Type", "application/octet-stream" if raw is not None else "application/json")
        self.send_header("Content-Length", str(len(data)))
        for key, value in (headers or {}).items():
            self.send_header(key, value)
        self.end_headers()
        if data:
            self.wfile.write(data)

    def _error(self, status, text):
        self.close_connection = True
        self._reply(status, {"error": text}, {"Connection": "close"})

    def _body(self, binary=False):
        lengths = self.headers.get_all("Content-Length", [])
        if self.headers.get_all("Transfer-Encoding") or len(lengths) != 1:
            raise ValueError("one Content-Length and no Transfer-Encoding required")
        if not re.fullmatch(r"[0-9]{1,10}", lengths[0]):
            raise ValueError("invalid Content-Length")
        length = int(lengths[0])
        if length > (FILE_LIMIT if binary else ACK_LIMIT if self.route == "/api/commands/ack" else JSON_LIMIT):
            raise OverflowError("fixture body limit exceeded")
        data = self.rfile.read(length)
        if len(data) != length:
            raise ValueError("incomplete request")
        if binary:
            return data
        if self.headers.get_content_type() != "application/json":
            raise ValueError("application/json required")
        return decoded(data)

    def do_GET(self):
        self._dispatch()

    def do_POST(self):
        self._dispatch()

    def _dispatch(self):
        fixture = self.server.fixture
        self.record = {"method": self.command, "path": self.path[:4096], "status": 0}
        with fixture.lock:
            fixture.requests.append(self.record)
        try:
            parts = urllib.parse.urlsplit(self.path)
            if len(self.path) > 4096 or not parts.path.startswith(fixture.prefix + "/"):
                self._error(404, "unknown route")
                return
            expected = "Bearer " + fixture.token
            tokens = self.headers.get_all("Authorization", [])
            if len(tokens) != 1 or not hmac.compare_digest(tokens[0].encode(), expected.encode()):
                self._error(401, "agent token required")
                return
            self.route = parts.path[len(fixture.prefix):]
            self.query = urllib.parse.parse_qs(parts.query, strict_parsing=True, max_num_fields=16) if parts.query else {}
            if any(len(values) != 1 for key, values in self.query.items() if key != "capability"):
                raise ValueError("duplicate query parameter")
            if self.command == "GET" and (self.headers.get("Content-Length", "0") != "0" or self.headers.get("Transfer-Encoding")):
                raise ValueError("GET body not accepted")
            with fixture.lock:
                faults = fixture.faults.get(self.route)
                fault = faults.popleft() if faults else None
            if fault and fault.kind == "delay":
                fixture.stopped.wait(float(fault.value))
            elif fault and fault.kind == "drop":
                self.record["fault"] = "drop"
                self.close_connection = True
                return
            elif fault and fault.kind == "redirect":
                self._reply(307, {}, {"Location": str(fault.value or "https://elsewhere.invalid/effector")})
                return
            elif fault and fault.kind == "ack_500":
                self._body()
                self._reply(500, {"error": "injected_ack_failure"})
                return
            if self.command == "GET":
                self._get(fault)
            else:
                self._post()
        except OverflowError as error:
            self._error(413, str(error))
        except (ValueError, KeyError, TypeError, RecursionError) as error:
            self._error(400, str(error))
        except (OSError, ssl.SSLError):
            self.close_connection = True

    def _arg(self, key):
        value = self.query.get(key, [""])[0]
        if not identifier(value):
            raise ValueError("invalid " + key)
        return value

    def _get(self, fault):
        fixture = self.server.fixture
        if self.route == "/api/config":
            self._reply(200, {"programs": []})
        elif self.route == "/api/agent_config":
            self._reply(200, {"auto_update_enabled": False, "transport_mode": "https_only",
                              "secure_server_url": fixture.url, "server_certificate_sha256": fixture.leaf_pin,
                              "server_certificate_sha256_next": ""})
        elif self.route == "/api/agent_version":
            self._error(404, "no MIND executable published by this fixture")
        elif self.route == "/api/commands/poll":
            agent = self._arg("agent_id")
            with fixture.lock:
                if agent not in fixture.agents:
                    self._error(404, "heartbeat required")
                    return
                commands = []
                full = self.query.get("at_capacity") == ["1"] or fixture.agents[agent].get("busy", False)
                if fixture.activated:
                    for command in fixture.commands.values():
                        if command["agent_id"] == agent and command["status"] == "pending":
                            if full and command["action"] != "cancel_command":
                                continue
                            command["status"] = "sent"
                            commands.append(copy.deepcopy(command))
                            break
                result = {"commands": commands, "parallel_limit": 1}
                if not fixture.activated:
                    result["activation_required"] = True
                if fault and fault.kind == "unknown_fields":
                    result["future_field"] = {"ignored": True}
                self._reply(200, result)
        elif self.route == "/api/commands/stream":
            agent = self._arg("agent_id")
            with fixture.lock:
                if agent not in fixture.agents:
                    self._error(404, "heartbeat required")
                    return
            self.record["status"] = 200
            self.send_response(200)
            self.send_header("Content-Type", "text/event-stream")
            self.send_header("Transfer-Encoding", "chunked")
            self.end_headers()
            self._chunk(b": connected\n\n")
            if fault and fault.kind == "cut_stream":
                self.wfile.write(b"40\r\nevent: comm")
                self.wfile.flush()
                self.close_connection = True
                return
            deadline = time.monotonic() + fixture.stream_seconds
            with fixture.changed:
                while not fixture.stopped.is_set() and time.monotonic() < deadline:
                    if fixture.activated and any(c["agent_id"] == agent and c["status"] == "pending" for c in fixture.commands.values()):
                        self._chunk(b"event: command\ndata: ready\n\n")
                        break
                    fixture.changed.wait(min(0.2, max(0, deadline - time.monotonic())))
            self.wfile.write(b"0\r\n\r\n")
            self.wfile.flush()
        elif self.route == "/api/commands/file":
            agent, command_id = self._arg("agent_id"), self._arg("command_id")
            with fixture.lock:
                command = fixture.commands.get(command_id)
                if not command or command["agent_id"] != agent or command["action"] != "deliver_file" or command["status"] != "sent":
                    self._error(409, "no sent delivery")
                    return
                self._download(fixture.files.get("command:" + command_id), fault)
        elif self.route.startswith("/api/deployment/download/"):
            package = self.route.rsplit("/", 1)[1]
            if not identifier(package):
                raise ValueError("invalid package id")
            with fixture.lock:
                self._download(fixture.files.get("package:" + package), fault)
        else:
            self._error(404, "unknown route")

    def _chunk(self, data):
        self.wfile.write(f"{len(data):x}\r\n".encode() + data + b"\r\n")
        self.wfile.flush()

    def _download(self, data, fault):
        if data is None:
            self._error(404, "fixture file not published")
        elif fault and fault.kind == "cut_download":
            self.record["status"] = 200
            self.send_response(200)
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            self.wfile.write(data[:max(0, min(len(data) - 1, int(fault.value or len(data) // 2)))])
            self.wfile.flush()
            self.close_connection = True
        else:
            self._reply(200, raw=data)

    def _post(self):
        fixture = self.server.fixture
        data = self._body(binary=self.route == "/api/commands/file")
        with fixture.changed:
            if self.route == "/api/heartbeat":
                agent = data.get("agent_id")
                if not identifier(agent):
                    raise ValueError("invalid agent_id")
                if agent not in fixture.agents and len(fixture.agents) >= MAX_AGENTS:
                    self._error(429, "fixture agent limit")
                    return
                fixture.agents[agent] = data
                fixture.beats.append(data)
                fixture.changed.notify_all()
                self._reply(200, {"status": "ok"})
            elif self.route == "/api/commands/ack":
                command_id, raw = data.get("command_id"), data.get("result")
                if not identifier(command_id) or not isinstance(raw, str):
                    raise ValueError("command_id and string result required")
                command = fixture.commands.get(command_id)
                if not command:
                    self._error(404, "unknown command")
                    return
                if command_id in fixture.results:
                    if fixture.results[command_id]["raw"] != raw:
                        self._error(409, "conflicting repeated ACK")
                        return
                elif command["status"] != "sent":
                    self._error(409, "command not sent")
                    return
                result = decoded(raw.encode())
                status = result.get("status", "failed")
                if status not in FINAL_STATES or (status == "completed" and result.get("exit_code", 0) != 0):
                    status = "failed"
                fixture.results[command_id] = {"status": status, "result": result, "raw": raw}
                command["status"] = status
                fixture.changed.notify_all()
                self._reply(200, {"status": "ok"})
            elif self.route == "/api/commands/file":
                agent, command_id = self._arg("agent_id"), self._arg("command_id")
                command = fixture.commands.get(command_id)
                if not command or command["agent_id"] != agent or command["action"] != "collect_file" or command["status"] != "sent":
                    self._error(409, "no sent collection")
                    return
                digest = hashlib.sha256(data).hexdigest()
                if self.query.get("sha256") != [digest]:
                    raise ValueError("sha256 mismatch")
                if command_id not in fixture.uploads and len(fixture.uploads) >= MAX_FILES:
                    self._error(429, "fixture file limit")
                    return
                fixture.uploads[command_id] = data
                self._reply(201, {"id": "artifact-" + command_id, "size": len(data), "sha256": digest})
            elif self.route == "/api/deployment/result":
                if len(fixture.deployment_results) >= MAX_COMMANDS:
                    self._error(429, "fixture result limit")
                    return
                fixture.deployment_results.append(data)
                self._reply(200, {"status": "ok"})
            elif self.route == "/api/telemetry":
                agent = self.headers.get("X-Effector-Agent-ID")
                if not identifier(agent) or agent not in fixture.agents:
                    self._error(404, "endpoint not found")
                    return
                problem = telemetry_problem(data)
                if problem:
                    self._error(400, "invalid telemetry sample: " + problem)
                    return
                if len(fixture.telemetry) >= MAX_SAMPLES:
                    fixture.telemetry.popleft()
                fixture.telemetry.append({"agent_id": agent, "sample": data})
                fixture.changed.notify_all()
                self._reply(204)
            else:
                self._error(404, "unknown route")


def telemetry_problem(sample):
    """What the Effector server's validateTelemetry (server/telemetry_store.go) would refuse in a schema 1 sample."""
    if not isinstance(sample, dict) or sample.get("schema") != 1:
        return "schema"
    if not isinstance(sample.get("sample_id"), str) or not re.fullmatch(r"[A-Za-z0-9._-]{1,96}", sample["sample_id"]):
        return "sample_id"
    interval = sample.get("interval_seconds")
    if not isinstance(interval, (int, float)) or not 0 < interval <= 3600:
        return "interval_seconds"
    if not isinstance(sample.get("sampled_at"), str) or not re.fullmatch(r"\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d(\.\d+)?(Z|[+-]\d\d:\d\d)", sample["sampled_at"]):
        return "sampled_at"
    system = sample.get("system")
    if not isinstance(system, dict) or not isinstance(system.get("logical_cpus"), int) or not 1 <= system["logical_cpus"] <= 65536:
        return "logical_cpus"
    lists = {"disks": 64, "interfaces": 64, "components": 128}
    for name, limit in lists.items():
        if not isinstance(sample.get(name), list) or len(sample[name]) > limit:
            return name
    for adapter in sample["interfaces"]:
        mac = adapter.get("mac_address", "")
        if mac and not re.fullmatch(r"[0-9a-fA-F]{2}(:[0-9a-fA-F]{2}){5}", mac):
            return "mac_address"
        for address in adapter.get("ip_addresses", []):
            if not re.fullmatch(r"\d{1,3}(\.\d{1,3}){3}", address):
                return "ip_addresses"

    def bad(value, key=""):
        if isinstance(value, dict):
            return next((k for k, v in value.items() if bad(v, k)), None)
        if isinstance(value, list):
            return len(value) > 256 or any(bad(v) for v in value)
        if isinstance(value, str):
            return len(value) > 512
        if isinstance(value, bool) or value is None:
            return False
        if isinstance(value, (int, float)):
            return value < 0 or value > 1e18 or (key.endswith("percent") and value > 100)
        return True
    found = bad(sample)
    return found and f"value {found}"


class EffectorFixture:
    """Host-only controls; never expose a second management API on the network."""

    def __init__(self, host="127.0.0.1", port=0, prefix="/effector", token="mind-effector-test-token", stream_seconds=0.2):
        if not re.fullmatch(r"(?:/[A-Za-z0-9_-]+)*", prefix) or not token or "\n" in token or "\r" in token:
            raise ValueError("invalid prefix or test token")
        if not 0 < stream_seconds <= 55:
            raise ValueError("stream duration must be in (0, 55]")
        self.host, self.port, self.prefix, self.token = host, port, prefix, token
        self.stream_seconds = stream_seconds
        self.lock = threading.RLock()
        self.changed = threading.Condition(self.lock)
        self.stopped = threading.Event()
        self.requests = collections.deque(maxlen=MAX_REQUESTS)
        self.errors = collections.deque(maxlen=32)
        self.commands, self.results, self.agents, self.files, self.uploads, self.faults = {}, {}, {}, {}, {}, {}
        self.deployment_results = []
        self.telemetry = collections.deque()
        self.beats = collections.deque(maxlen=MAX_REQUESTS)
        self.activated = True
        self.server = self.thread = self.directory = None

    def start(self):
        if self.server is not None:
            raise RuntimeError("fixture already started")
        self.directory = tempfile.TemporaryDirectory(prefix="mind-effector-")
        directory = pathlib.Path(self.directory.name)
        self.certificate, self.key = directory / "server.pem", directory / "server.key"
        try:
            subprocess.run(["openssl", "req", "-x509", "-newkey", "ec", "-pkeyopt", "ec_paramgen_curve:P-256",
                            "-nodes", "-days", "1", "-subj", "/CN=localhost", "-addext",
                            "subjectAltName=DNS:localhost,IP:127.0.0.1,IP:10.0.2.2", "-addext",
                            "basicConstraints=critical,CA:FALSE", "-addext", "extendedKeyUsage=serverAuth",
                            "-keyout", str(self.key), "-out", str(self.certificate)],
                           check=True, capture_output=True, timeout=20)
            der = ssl.PEM_cert_to_DER_cert(self.certificate.read_text())
            self.leaf_pin = hashlib.sha256(der).hexdigest()
            context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
            context.minimum_version = ssl.TLSVersion.TLSv1_3
            context.load_cert_chain(self.certificate, self.key)
            self.server = _Server((self.host, self.port), self, context)
            self.port = self.server.server_address[1]
            self.url = f"https://{self.host}:{self.port}{self.prefix}"
            self.thread = threading.Thread(target=self.server.serve_forever, kwargs={"poll_interval": 0.05}, daemon=True)
            self.thread.start()
            return self
        except BaseException:
            self.close()
            raise

    def close(self):
        self.stopped.set()
        with self.changed:
            self.changed.notify_all()
        if self.server:
            if self.thread and self.thread.is_alive():
                self.server.shutdown()
            self.server.server_close()
        if self.thread:
            self.thread.join(timeout=5)
        if self.directory:
            self.directory.cleanup()
        self.server = self.thread = self.directory = None

    def __enter__(self):
        return self.start()

    def __exit__(self, *_):
        self.close()

    def queue(self, agent, action, payload="", program="", command_id=None):
        with self.changed:
            if len(self.commands) >= MAX_COMMANDS:
                raise ValueError("fixture command limit")
            command_id = command_id or f"cmd-{len(self.commands) + 1}"
            if not identifier(agent) or not identifier(command_id) or command_id in self.commands:
                raise ValueError("invalid or reused command id")
            command = {"id": command_id, "agent_id": agent, "action": action, "program_name": program,
                       "new_hostport": payload if isinstance(payload, str) else encoded(payload).decode(), "status": "pending"}
            if len(encoded(command)) > 16384:
                raise ValueError("fixture command size limit")
            self.commands[command_id] = command
            self.changed.notify_all()
            return command_id

    def redeliver(self, command_id):
        with self.changed:
            self.commands[command_id]["status"] = "pending"
            self.changed.notify_all()

    def operation(self, command_id):
        with self.lock:
            return copy.deepcopy(self.results.get(command_id))

    def publish(self, data, *, command_id=None, package_id=None):
        if (command_id is None) == (package_id is None) or not isinstance(data, bytes) or len(data) > FILE_LIMIT:
            raise ValueError("one fixture file id and at most 1 MiB required")
        name = command_id if command_id is not None else package_id
        if not identifier(name):
            raise ValueError("invalid fixture file id")
        key = ("command:" if command_id is not None else "package:") + name
        with self.lock:
            if key not in self.files and len(self.files) >= MAX_FILES:
                raise ValueError("fixture file limit")
            self.files[key] = data
        return {"size": len(data), "sha256": hashlib.sha256(data).hexdigest()}

    def inject(self, route, kind, value=None):
        kinds = {"delay", "drop", "redirect", "ack_500", "unknown_fields", "cut_stream", "cut_download"}
        if kind not in kinds or not route.startswith("/api/"):
            raise ValueError("unknown fault")
        if kind == "delay" and not 0 <= float(value) <= 5:
            raise ValueError("fixture delay limit")
        with self.lock:
            if len(self.faults) >= 16 and route not in self.faults:
                raise ValueError("fixture fault route limit")
            faults = self.faults.setdefault(route, collections.deque())
            if len(faults) >= 16:
                raise ValueError("fixture fault limit")
            faults.append(Fault(kind, value))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=0)
    parser.add_argument("--prefix", default="/effector")
    parser.add_argument("--stream-seconds", type=float, default=55)
    args = parser.parse_args()
    with EffectorFixture(args.host, args.port, args.prefix, stream_seconds=args.stream_seconds) as fixture:
        print(json.dumps({"contract": CONTRACT, "url": fixture.url, "leaf_sha256": fixture.leaf_pin,
                          "certificate": str(fixture.certificate)}), flush=True)
        with contextlib.suppress(KeyboardInterrupt):
            while not fixture.stopped.wait(1):
                pass


if __name__ == "__main__":
    main()
