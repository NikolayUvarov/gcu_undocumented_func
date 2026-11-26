"""scripts/board_stage1.py on consoles (100004-MOB-0012): one recorded in QEMU, one composed in a board's shape, and broken
ones made from the recording line by line; read from a file, from standard input, from a TCP line, from effector485's
console API and from an Effector server's serial session, the last three by stand-ins that answer each typed command with
the recording's answer. No console here is a board's: what passes says how the script reads, not how a board boots (MC-12.1)."""
import base64
import http.server
import json
from pathlib import Path
import re
import socket
import subprocess
import sys
import tempfile
import threading
import time
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
from scripts import board_stage1  # noqa: E402

DATA = ROOT / "tests/data/stage1"
# Recorded as the script read it (--save) on 2026-11-26: QEMU 8.2.2 `virt,acpi=off,gic-version=2,virtualization=on`, four
# Cortex-A53, 512 MiB, U-Boot 2025.10's UEFI (scripts/fetch_uboot.sh), the Zero 3's card from board_image.py, with --type.
RECORDED = DATA / "qemu-virt-uboot-a53-512m.txt"
# Composed from the formats in the code, in a board's shape (U-Boot's SPL, a 16550, EL2): no board printed it.
COMPOSED = DATA / "composed-h5-shape.txt"
GOOD = RECORDED
TOOL = [sys.executable, str(ROOT / "scripts/board_stage1.py")]
COMMANDS = {c for c in board_stage1.COMMANDS if isinstance(c, str)}


def check(text, cpus=4):
    stage = board_stage1.Stage1(cpus=cpus)
    stage.feed(text)
    stage.end()
    return stage


def edited(pattern, replacement=None, after=None, count=1):
    """The good recording with the first `count` lines matching `pattern` replaced, removed (None), or a line put after them."""
    lines, done = GOOD.read_text().split("\n"), 0
    out = []
    for line in lines:
        if done < count and re.search(pattern, line):
            done += 1
            if after is not None:
                out += [line, after]
            elif replacement is not None:
                out.append(re.sub(pattern, replacement, line))
            continue
        out.append(line)
    assert done == count, (pattern, done)
    return "\n".join(out)


def cut(pattern):
    """The good recording up to the line before the first matching `pattern`."""
    text = GOOD.read_text()
    return text[:re.search(pattern, text, re.M).start()]


class Board:
    """The recording as a board that answers: the boot up to its first prompt, then each command's recorded answer and the
    next prompt; `time` answered from this side's clock, `rate` times as fast, so the script compares a live clock."""

    def __init__(self, rate=1.0):
        text = GOOD.read_text().replace("\r", "")
        first = text.index("MIND> ")
        self.boot, rest = text[:first + 6], text[first + 6:]
        self.answers, self.typed, self.rate, self.start = {}, [], rate, time.monotonic()
        for segment in rest.split("MIND> "):
            # The typed command's echo starts the segment, unless a service's line came between the prompt and it.
            lines = segment.split("\n")
            at = next((i for i, line in enumerate(lines) if line.strip() in COMMANDS), None)
            if at is not None:
                self.answers.setdefault(lines[at].strip(), "\n".join(lines[at + 1:]))
        self.lock = threading.Lock()

    def answer(self, command):
        with self.lock:
            self.typed.append((command, time.monotonic()))
        if command == "time":
            ms = int((time.monotonic() - self.start) * 1000 * self.rate) + 20000
            return f"time\nTIME: --:--:-- UPTIME MS={ms} MONOTONIC NS={ms * 1000000} RESOLUTION NS=16 TSC HZ=62500000\nMIND> "
        return f"{command}\n{self.answers[command]}" + ("" if command == "reboot" else "MIND> ")


class Lines(unittest.TestCase):
    def test_the_good_consoles(self):
        for console in (RECORDED, COMPOSED):
            with self.subTest(console=console.name):
                stage = check(console.read_text())
                text, missing = stage.report("good")
                self.assertEqual(missing, 0, text)
                self.assertIn("CPUS 4", stage.board[1])
                self.assertIn("ENTERED AT EL2", stage.board[1])
        # U-Boot's SPL comes first on a board: the banner is U-Boot proper's, and the next boot's SPL shows the reboot.
        stage = check(COMPOSED.read_text())
        self.assertTrue(stage.met["u-boot"][1].startswith("U-Boot 2026.10 "), stage.met["u-boot"])
        self.assertIn("then U-Boot SPL 2026.10", stage.met["reboot"][1])

    def broken(self, text, name, why):
        stage = check(text)
        report, missing = stage.report("broken")
        self.assertGreater(missing, 0, report)
        self.assertNotIn(name, stage.met, report)
        self.assertRegex(next(row for row in report.splitlines() if row.startswith(f"MISSING  {name}")), why)
        return stage

    def test_broken_recordings(self):
        self.broken(edited(r"^CPU=2 .*"), "cpus", "answered 3 CPUs, not 4")
        self.broken(edited(r"ONLINE=true TICKS=\d+", "ONLINE=false TICKS=0", count=1), "cpus", "3 of 4 CPUs online")
        self.broken(edited(r"MIND CORE KERNEL: INIT STARTED", after="MIND CORE KERNEL: A CPU DID NOT COME ONLINE"), "cpus", "A CPU DID NOT COME ONLINE")
        self.broken(edited(r"IMAGES CHECKED", after="BOOT ERROR: kernel.elf: the image's hash is not the manifest's"), "loader", "BOOT ERROR")
        stage = self.broken(cut(r"^MIND CORE KERNEL: INIT STARTED") + "MIND CORE KERNEL: INIT STARTED\n\"Synchronous Abort\" handler, esr 0x56000000\n",
                            "kernel", "Synchronous Abort")
        self.assertNotIn("prompt", stage.met)
        self.broken(edited(r"UPTIME MS=\d+", "UPTIME MS=1000", count=2), "timer", "did not go on")
        self.broken(cut(r"REBOOTING"), "reboot", "no `MIND CORE KERNEL: REBOOT VIA")
        self.broken(edited(r"TIMER PPI 27 CPUS 4", "TIMER PPI 27 CPUS 2"), "cpus", "names 2 CPUs, not 4")
        self.broken(edited(r"^RANGES=\d+ FREE_RAM=\d+K", "RANGES=0 FREE_RAM=0K"), "memory", "RANGES=0")
        self.broken(cut(r"^MIND> |MIND> "), "prompt", "no shell prompt")

    def test_read_after_u_boot_started(self):
        text = GOOD.read_text()
        stage = self.broken(text[text.index("BOOT: "):], "u-boot", "no line `U-Boot")
        self.assertIn("loader", stage.met)
        self.assertTrue(any("no U-Boot banner before it" in note for note in stage.notes), stage.notes)

    def test_fed_a_byte_at_a_time(self):
        stage = board_stage1.Stage1()
        for ch in GOOD.read_text():
            stage.feed(ch)
        stage.end()
        self.assertEqual(stage.report("bytes")[1], 0, stage.report("bytes")[0])
        self.assertEqual({k: v[0] for k, v in stage.met.items()}, {k: v[0] for k, v in check(GOOD.read_text()).met.items()}, "the same lines decide")

    def test_the_command_from_a_file_and_standard_input(self):
        done = subprocess.run([*TOOL, "--file", str(GOOD)], capture_output=True, text=True)
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
        self.assertIn("8 OF 8 MET", done.stdout)
        done = subprocess.run([*TOOL, "--file", "-"], input=cut(r"REBOOTING"), capture_output=True, text=True)
        self.assertEqual(done.returncode, 1, done.stdout + done.stderr)
        self.assertIn("7 OF 8 MET", done.stdout)
        self.assertIn("STAGE 1 FROM standard input", done.stdout)
        done = subprocess.run([*TOOL, "--file", str(GOOD), "--type"], capture_output=True, text=True)
        self.assertEqual(done.returncode, 2)
        self.assertIn("--type needs a source that takes input", done.stderr)
        done = subprocess.run([*TOOL, "--effector", "s1"], capture_output=True, text=True, env={"PATH": "/usr/bin:/bin"})
        self.assertEqual(done.returncode, 2)
        self.assertIn("--effector needs the key in $EFFECTOR_KEY", done.stderr)


class Live(unittest.TestCase):
    """The sources that take input, against stand-ins: a TCP line, effector485's API and an Effector server's."""

    def setUp(self):
        self.dir = tempfile.TemporaryDirectory(prefix="board-stage1-")
        self.addCleanup(self.dir.cleanup)

    def typed_in_order(self, board):
        commands = [c for c, _ in board.typed]
        self.assertEqual(commands, ["cpus", "physmap", "free", "time", "time", "reboot"])
        times = [t for c, t in board.typed if c == "time"]
        self.assertGreaterEqual(times[1] - times[0], 2.9, "the two answers of time some seconds apart")

    def tcp(self, board):
        server = socket.socket()
        server.bind(("127.0.0.1", 0))
        server.listen(1)
        port = server.getsockname()[1]

        def serve():
            connection, _ = server.accept()
            with connection:
                connection.sendall(board.boot.encode())
                line = b""
                while True:
                    data = connection.recv(64)
                    if not data:
                        return
                    line += data
                    while b"\r" in line:
                        command, _, line = line.partition(b"\r")
                        connection.sendall(board.answer(command.decode()).encode())
        thread = threading.Thread(target=serve, daemon=True)
        thread.start()
        self.addCleanup(server.close)
        self.addCleanup(thread.join, 5)
        return f"127.0.0.1:{port}"

    def test_a_tcp_line_typed_on(self):
        board = Board()
        save = Path(self.dir.name) / "console.txt"
        done = subprocess.run([*TOOL, "--tcp", self.tcp(board), "--type", "--timeout", "60", "--save", str(save)], capture_output=True, text=True, timeout=90)
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
        self.typed_in_order(board)
        self.assertRegex(done.stdout, r"MET      timer   .* on the kernel's clock, \d+\.\d+ s on the host's")
        self.assertEqual(check(save.read_text()).report("saved")[1], 0, "the console saved as read checks the same")

    def test_a_clock_at_half_speed(self):
        board = Board(rate=0.5)
        done = subprocess.run([*TOOL, "--tcp", self.tcp(board), "--type", "--timeout", "60"], capture_output=True, text=True, timeout=90)
        self.assertEqual(done.returncode, 1, done.stdout + done.stderr)
        self.assertRegex(done.stdout, r"MISSING  timer   .* on the kernel's clock, but \d+\.\d+ s on the host's")

    def http(self, board, handler):
        """An HTTP stand-in on the loopback: `handler(method, path, query, body, headers)` gives (status, answer)."""
        class Handler(http.server.BaseHTTPRequestHandler):
            def log_message(self, *args):
                pass

            def reply(self, method):
                path, _, query = self.path.partition("?")
                fields = dict(pair.split("=", 1) for pair in query.split("&") if "=" in pair)
                body = json.loads(self.rfile.read(int(self.headers.get("Content-Length") or 0)) or b"null")
                status, answer = handler(method, path, fields, body, self.headers)
                data = json.dumps(answer).encode()
                self.send_response(status)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(data)))
                self.end_headers()
                self.wfile.write(data)

            def do_GET(self):
                self.reply("GET")

            def do_POST(self):
                self.reply("POST")
        server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        threading.Thread(target=server.serve_forever, daemon=True).start()
        self.addCleanup(server.server_close)
        self.addCleanup(server.shutdown)
        return f"http://127.0.0.1:{server.server_address[1]}"

    def test_effector485s_console(self):
        board, ring, seqs = Board(), bytearray(), []
        ring += board.boot.encode()
        key = Path(self.dir.name) / "key"
        key.write_text("k485\n")

        def handler(method, path, query, body, headers):
            if headers.get("X-API-Key") != "k485":
                return 401, {"error": "key"}
            if method == "GET" and path == "/api/v1/console" and query.get("agent_id") == "mind_board":
                start = int(query["from"])
                data = bytes(ring[start:start + 4096])
                return 200, {"from": start, "next": start + len(data), "gap": 0, "data": base64.b64encode(data).decode(),
                             "pieces": [{"offset": start, "time_ms": int(time.time() * 1000)}] if data else [], "mode": "console"}
            if method == "POST" and path == "/api/v1/console/input" and body.get("agent_id") == "mind_board":
                written = not seqs or body["seq"] > seqs[-1]
                if written:
                    seqs.append(body["seq"])
                    ring.extend(board.answer(body["text"].rstrip("\r")).encode())
                return 200, {"written": written, "mode": "console"}
            return 404, {"error": path}
        url = self.http(board, handler)
        done = subprocess.run([*TOOL, "--effector485", url, "--agent", "mind_board", "--key-file", str(key), "--type", "--timeout", "60"],
                              capture_output=True, text=True, timeout=90)
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
        self.typed_in_order(board)
        self.assertEqual(seqs, sorted(set(seqs)), "each input's seq above the last")

    def test_an_effector_servers_serial_session(self):
        board, chunks, inputs = Board(), [], []

        def add(text):
            offset = sum(len(c["raw"]) for c in chunks)
            chunks.append({"offset": offset, "time_ms": int(time.time() * 1000), "raw": text.encode()})
        add(board.boot)

        def handler(method, path, query, body, headers):
            if headers.get("Authorization") != "Bearer kfx":
                return 401, {"error": "key"}
            if method == "GET" and path == "/api/v1/serial/output" and query.get("session_id") == "sid1":
                assert "pin" not in query and "key" not in query
                start = int(query["from"])
                out = [{"offset": c["offset"], "time_ms": c["time_ms"], "data": base64.b64encode(c["raw"]).decode()} for c in chunks if c["offset"] >= start]
                end = chunks[-1]["offset"] + len(chunks[-1]["raw"])
                if not out:
                    time.sleep(min(int(query.get("wait_ms", 0)), 200) / 1000)
                return 200, {"session": {"state": "running"}, "from": start, "next": end if out else start, "gap": 0, "chunks": out}
            if method == "POST" and path == "/api/v1/serial/input":
                if (body.get("session_id"), body.get("lease_id"), body.get("pin")) != ("sid1", "lease1", "86420713"):
                    return 403, {"error": "lease"}
                inputs.append(body)
                add(board.answer(body["text"].rstrip("\r")))
                return 200, {"session_id": "sid1", "write": {"seq": body["seq"], "status": "queued"}}
            return 404, {"error": path}
        url = self.http(board, handler)
        env = {"PATH": "/usr/bin:/bin", "EFFECTOR_KEY": "kfx", "EFFECTOR_SERIAL_PIN": "86420713"}
        done = subprocess.run([*TOOL, "--effector", "sid1", "--server", url, "--lease", "lease1", "--type", "--timeout", "60"],
                              capture_output=True, text=True, timeout=90, env=env)
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
        self.typed_in_order(board)
        self.assertTrue(all(i["pace_ms"] == 20 and i["reason"] for i in inputs))
        self.assertNotIn("86420713", done.stdout + done.stderr)


if __name__ == "__main__":
    unittest.main()
