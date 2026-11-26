#!/usr/bin/env python3
"""Stage 1's criteria checked from a board's console (100004-MOB-0012): U-Boot's banner, our loader's lines, the kernel's
lines, every CPU online, the timer, the memory map, the shell's prompt and a reboot, each said met or missing with the line
that showed it. Exits 0 when all are met, 1 when one is missing, 2 when the console could not be read.

The console is read from a file, from standard input as a stream (`--file -`), from a TCP line (QEMU's `-serial tcp:`, a
serial server), from effector485's console API, or from an Effector server's serial session. With `--type`, on a source
that takes input, it types at the shell's prompt what shows the rest: `cpus`, `physmap`, `free`, `time` twice 3 s apart,
then `reboot`. From a file, those answers must be in the console already (docs/boards/allwinner-first-boot.md).

What it reads is evidence for the configuration the console came from only (MC-12.1): the board, its card, the adapter, the
host. A console of QEMU says nothing about a board.
"""
import argparse
import base64
import json
import os
from pathlib import Path
import re
import select
import socket
import sys
import time
import urllib.parse
import urllib.request

ANSI = re.compile(r"\x1b\[[0-9;?]*[A-Za-z]|\x1b[()][A-Za-z0-9]")
UBOOT = re.compile(r"^U-Boot (\d{4}\.\d{2}\S*)")  # U-Boot proper's banner; the SPL's says `U-Boot SPL`
SPL = re.compile(r"^U-Boot SPL (\d{4}\.\d{2}\S*)")
PROMPT = re.compile(r"MIND(?: [\w:/.-]+)?> ")
CHECKED = re.compile(r"KEY \S+.* VERIFIED, (\d+) IMAGES CHECKED")
BOARD = re.compile(r"MIND CORE KERNEL: BOARD (?:FROM THE DEVICE TREE: )?GICV(\d).* TIMER PPI (\d+) CPUS (\d+)")
INIT = re.compile(r"MIND CORE KERNEL: INIT STARTED")
CPU = re.compile(r"^CPU=(\d+) APIC=\S+ ONLINE=(true|false) TICKS=(\d+)")
PHYSMAP = re.compile(r"^RANGES=(\d+) FREE_RAM=(\d+)K")
MEMORY = re.compile(r"^MEMORY: (\d+) MIB")
UPTIME = re.compile(r"UPTIME MS=(\d+)")
REBOOT = re.compile(r"MIND CORE KERNEL: REBOOT VIA (.+)")
# Lines that fail a criterion whatever came before: the loader's and the kernel's failures, U-Boot's exception handler.
FAILS = (("loader", re.compile(r"BOOT (ERROR|PANIC): ")),
         ("kernel", re.compile(r"KERNEL PANIC: |\"Synchronous Abort\" handler|MIND CORE KERNEL: DEVICE TREE: NO (GIC|PSCI|CONSOLE)")),
         ("cpus", re.compile(r"MIND CORE KERNEL: (A CPU DID NOT COME ONLINE|PSCI CPU_ON REFUSED)")))
CRITERIA = ("u-boot", "loader", "kernel", "cpus", "timer", "memory", "prompt", "reboot")
COMMANDS = ("cpus", "physmap", "free", "time", 3.0, "time", "reboot")  # a number: seconds to wait before the next
HINTS = {"u-boot": "no line `U-Boot 20YY.MM` before our loader's",
         "loader": "no line `... KEY ... VERIFIED, N IMAGES CHECKED` from our loader",
         "kernel": "no line `MIND CORE KERNEL: BOARD ...` then `MIND CORE KERNEL: INIT STARTED`",
         "cpus": "no answer of `cpus` (type cpus at the prompt, or --type)",
         "timer": "no two answers of `time` (type time twice, some seconds apart, or --type)",
         "memory": "no answer of `physmap` (type physmap at the prompt, or --type)",
         "prompt": "no shell prompt `MIND> `",
         "reboot": "no `MIND CORE KERNEL: REBOOT VIA ...` after the prompt, then U-Boot again (type reboot, or --type)"}


class Stage1:
    """Takes the console's text as it comes and decides each criterion on the first boot's lines."""

    def __init__(self, cpus=4, tolerance=0.1):
        self.cpus, self.tolerance = cpus, tolerance
        self.number, self.tail, self.tail_counted = 0, "", False
        self.boot = 0  # 1 from the first banner on; 2 once the reboot is seen
        self.met, self.failed, self.notes = {}, {}, []
        self.board = self.rebooting = None
        self.cpu_rows, self.uptimes = {}, []
        self.prompts = 0

    def feed(self, text, at=None):
        """Console text, `at` the host's time it came (seconds) or None; returns how many new prompts it held."""
        before = self.prompts
        *lines, self.tail = (self.tail + ANSI.sub("", text).replace("\r", "")).split("\n")
        for index, line in enumerate(lines):
            self.number += 1
            self.line(line, at, counted=self.tail_counted and index == 0)
        if lines:
            self.tail_counted = False
        # The prompt ends no line: it counts while it is the tail, and the line it begins does not count it again.
        if not self.tail_counted and PROMPT.search(self.tail):
            self.tail_counted = True
            self.prompt(self.number + 1, self.tail)
        return self.prompts - before

    def end(self):
        if self.tail:
            self.number += 1
            self.line(self.tail, None, counted=self.tail_counted)
            self.tail = ""

    def done(self):
        return "reboot" in self.met or all(name in self.met for name in CRITERIA)

    def say(self, name, number, text):
        if name not in self.met and name not in self.failed:
            self.met[name] = (number, text.strip())

    def fail(self, name, number, text):
        self.met.pop(name, None)
        self.failed.setdefault(name, (number, text.strip()))

    def prompt(self, number, text):
        if self.boot == 1 and not self.rebooting:
            self.prompts += 1
            self.say("prompt", number, text)

    def line(self, line, at, counted):
        n, text = self.number, line.strip()
        if self.boot == 2:
            return
        if UBOOT.match(text) or SPL.match(text):
            if self.rebooting:
                self.boot = 2
                self.say("reboot", n, f"line {self.rebooting[0]}: {self.rebooting[1]}; then {text}")
            elif self.boot == 0:
                self.boot = 1
            if self.boot == 1 and UBOOT.match(text):
                self.say("u-boot", n, text)
            return
        if self.boot == 0:
            if not (CHECKED.search(text) or BOARD.search(text)):
                return
            self.notes.append(f"line {n}: our loader's or the kernel's line with no U-Boot banner before it: read after U-Boot started?")
            self.boot = 1
        if self.rebooting:
            return
        for name, pattern in FAILS:
            if pattern.search(text):
                self.fail(name, n, text)
        if not counted and PROMPT.search(line):
            self.prompt(n, text)
        if CHECKED.search(text):
            self.say("loader", n, text)
        elif m := BOARD.search(text):
            self.board = (n, text)
            if int(m[3]) != self.cpus:
                self.fail("cpus", n, f"the board names {m[3]} CPUs, not {self.cpus}: {text}")
        elif INIT.search(text):
            if self.board:
                self.say("kernel", n, f"{text}, after the board's line {self.board[0]}")
        elif m := CPU.match(text):
            self.cpus_answer(n, text, int(m[1]), m[2] == "true", int(m[3]))
        elif m := PHYSMAP.match(text):
            (self.say if int(m[1]) > 0 and int(m[2]) > 0 else self.fail)("memory", n, text)
        elif MEMORY.match(text):
            self.notes.append(f"line {n}: {text}")
        elif m := UPTIME.search(text):
            self.uptimes.append((n, int(m[1]), at))
            self.timer()
        elif REBOOT.search(text) and "prompt" in self.met:
            self.rebooting = (n, text)

    def cpus_answer(self, n, text, index, online, ticks):
        """`cpus` answers a row per CPU: each one online, and its timer ticking."""
        self.cpu_rows[index] = (online, ticks)
        if len(self.cpu_rows) > self.cpus:
            self.fail("cpus", n, f"{len(self.cpu_rows)} CPUs answer, not {self.cpus}: {text}")
        elif len(self.cpu_rows) == self.cpus:
            up = sum(1 for o, _ in self.cpu_rows.values() if o)
            ticking = sum(1 for o, t in self.cpu_rows.values() if o and t > 0)
            if up == ticking == self.cpus:
                self.say("cpus", n, f"{up} of {self.cpus} CPUs ONLINE=true with TICKS above 0; the last: {text}")
            else:
                self.fail("cpus", n, f"{up} of {self.cpus} CPUs online, {ticking} ticking: {text}")

    def timer(self):
        """Two answers of `time`: the kernel's clock went on, as fast as the host's within the tolerance when its times are known."""
        if len(self.uptimes) < 2:
            return
        (_, ms0, at0), (n1, ms1, at1) = self.uptimes[0], self.uptimes[-1]
        kernel = (ms1 - ms0) / 1000
        if ms1 <= ms0:
            self.fail("timer", n1, f"UPTIME MS={ms0}, then {ms1}: the kernel's clock did not go on")
        elif at0 is None or at1 is None:
            self.say("timer", n1, f"UPTIME MS={ms0}, then {ms1}: {kernel:.2f} s on the kernel's clock (no host times to compare)")
        elif abs(kernel - (at1 - at0)) <= self.tolerance * (at1 - at0) + 0.5:
            self.say("timer", n1, f"UPTIME MS={ms0}, then {ms1}: {kernel:.2f} s on the kernel's clock, {at1 - at0:.2f} s on the host's")
        else:
            self.fail("timer", n1, f"UPTIME MS={ms0}, then {ms1}: {kernel:.2f} s on the kernel's clock, but {at1 - at0:.2f} s on the host's")

    def report(self, source):
        rows, missing = [], 0
        for name in CRITERIA:
            if name in self.met:
                rows.append(f"MET      {name:<7} line {self.met[name][0]}: {self.met[name][1]}")
                continue
            missing += 1
            number, text = self.failed.get(name, (None, None))
            hint = f"`cpus` answered {len(self.cpu_rows)} CPUs, not {self.cpus}" if name == "cpus" and self.cpu_rows else HINTS[name]
            rows.append(f"MISSING  {name:<7} " + (f"line {number}: {text}" if number else hint))
        head = [f"STAGE 1 FROM {source}: {len(CRITERIA) - missing} OF {len(CRITERIA)} MET, {self.number} LINES READ"]
        if self.board:
            head.append(f"THE BOARD'S LINE {self.board[0]}: {self.board[1]}")
        return "\n".join([*head, *rows, *self.notes]), missing


class Typist:
    """Types COMMANDS through `send`, one at each new prompt, after the wait a number before it asks."""

    def __init__(self, send):
        self.send, self.left, self.due = send, list(COMMANDS), None

    def tick(self, now, prompted):
        if prompted and self.due is None and self.left:
            self.due = now + (self.left.pop(0) if isinstance(self.left[0], float) else 0.0)
        if self.due is not None and now >= self.due and self.left:
            self.due = None
            self.send(self.left.pop(0) + "\r")


def run(stage, chunks, typist=None, timeout=None, save=None):
    """Feeds the stage from `chunks`, (text, time) or None while nothing came, until done, the end or the timeout."""
    end = time.monotonic() + timeout if timeout else None
    for item in chunks:
        prompted = 0
        if item is not None:
            if save:
                save.write(item[0])
                save.flush()
            prompted = stage.feed(*item)
        if typist:
            typist.tick(time.monotonic(), prompted)
        if stage.done() or (end and time.monotonic() > end):
            break
    stage.end()


def from_stream(stream):
    for line in iter(stream.readline, ""):
        yield line, None


def from_tcp(address, typed):
    host, _, port = address.rpartition(":")
    for attempt in range(40):  # a line that is starting, as QEMU's is, answers within 10 s
        try:
            connection = socket.create_connection((host or "127.0.0.1", int(port)), timeout=30)
            break
        except ConnectionRefusedError:
            if attempt == 39:
                raise
            time.sleep(0.25)
    connection.settimeout(None)
    typed.append(lambda text: connection.sendall(text.encode()))
    with connection:
        while True:
            if not select.select([connection], [], [], 0.25)[0]:
                yield None
                continue
            data = connection.recv(65536)
            if not data:
                return
            yield data.decode("utf-8", "replace"), time.time()


def request(url, headers, body=None):
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(url, data=data, headers={**headers, **({"Content-Type": "application/json"} if data else {})})
    with urllib.request.urlopen(req, timeout=60) as answer:
        return json.loads(answer.read() or b"{}")


def from_effector485(base, agent, key, typed):
    """effector485's console (docs/link/effector485.md, API 1.2): read from offset 0 every 250 ms; input with a growing `seq`."""
    headers = {"X-API-Key": key} if key else {}
    seq = [int(time.time() * 1000)]  # above any number an earlier run took

    def send(text):
        seq[0] += 1
        request(f"{base}/api/v1/console/input", headers, {"agent_id": agent, "seq": seq[0], "text": text, "pace_ms": 20})
    typed.append(send)
    offset = 0
    while True:
        answer = request(f"{base}/api/v1/console?" + urllib.parse.urlencode({"agent_id": agent, "from": offset, "max_bytes": 65536}), headers)
        data = base64.b64decode(answer.get("data") or "")
        offset = answer.get("next", offset + len(data))
        if data:
            pieces = answer.get("pieces") or []
            yield data.decode("utf-8", "replace"), (pieces[-1]["time_ms"] / 1000 if pieces else time.time())
        else:
            time.sleep(0.25)
            yield None


def from_effector(server, session, key, lease, pin, typed):
    """An Effector server's serial session (sst-test-deploy docs/serial-ports.md): its output long-polled from offset 0,
    input as the lease's holder; the key and the PIN go in no URL."""
    headers = {"Authorization": f"Bearer {key}"}
    seq = [int(time.time() * 1000)]

    def send(text):
        seq[0] += 1
        request(f"{server}/api/v1/serial/input", headers, {"session_id": session, "lease_id": lease, "pin": pin, "seq": seq[0],
                                                           "text": text, "pace_ms": 20, "reason": "stage 1's run (100004-MOB-0012)"})
    if lease and pin:
        typed.append(send)
    offset = 0
    while True:
        query = urllib.parse.urlencode({"session_id": session, "from": offset, "wait_ms": 2000, "max_bytes": 65536})
        answer = request(f"{server}/api/v1/serial/output?{query}", headers)
        chunks = answer.get("chunks") or []
        for chunk in chunks:
            yield base64.b64decode(chunk["data"]).decode("utf-8", "replace"), chunk["time_ms"] / 1000
        offset = answer.get("next", offset)
        if not chunks:
            if (answer.get("session") or {}).get("state") in ("closed", "failed"):
                return
            yield None


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--file", help="a console's text; - reads standard input as a stream")
    source.add_argument("--tcp", metavar="HOST:PORT", help="a console on a TCP line (QEMU's -serial tcp:HOST:PORT,server=on)")
    source.add_argument("--effector485", metavar="URL", help="effector485's API, such as http://127.0.0.1:8485 (with --agent)")
    source.add_argument("--effector", metavar="SESSION", help="an Effector server's serial session (POST /api/v1/serial/sessions)")
    parser.add_argument("--agent", help="with --effector485: the machine's ID, or `line` on a cable of several")
    parser.add_argument("--key-file", help="with --effector485: its key file (else $EFFECTOR485_KEY_FILE)")
    parser.add_argument("--server", default="https://hesere.uz/effector", help="with --effector: the server; the key in $EFFECTOR_KEY")
    parser.add_argument("--lease", help="with --effector --type: the port's lease; its PIN in $EFFECTOR_SERIAL_PIN")
    parser.add_argument("--type", action="store_true", help="type cpus, physmap, free, time, time and reboot at the prompt")
    parser.add_argument("--cpus", type=int, default=4, help="the CPUs the board has (default 4)")
    parser.add_argument("--timeout", type=float, default=600, help="seconds to read a live console (default 600)")
    parser.add_argument("--save", help="also write the console's text as read to this file")
    args = parser.parse_args()
    if args.type and args.file:
        parser.error("--type needs a source that takes input: --tcp, --effector485 or --effector")
    if args.effector485 and not args.agent:
        parser.error("--effector485 needs --agent")
    if args.effector and not os.environ.get("EFFECTOR_KEY"):
        parser.error("--effector needs the key in $EFFECTOR_KEY")
    if args.effector and args.type and not (args.lease and os.environ.get("EFFECTOR_SERIAL_PIN")):
        parser.error("--effector --type needs --lease and the lease's PIN in $EFFECTOR_SERIAL_PIN")
    typed, label = [], "standard input" if args.file == "-" else (args.file or args.tcp or args.effector485 or f"Effector session {args.effector}")
    stage = Stage1(cpus=args.cpus)
    save = open(args.save, "w", encoding="utf-8") if args.save else None
    try:
        if args.file:
            stream = sys.stdin if args.file == "-" else open(args.file, encoding="utf-8", errors="replace")
            run(stage, from_stream(stream), save=save)
        else:
            if args.tcp:
                chunks = from_tcp(args.tcp, typed)
            elif args.effector485:
                key_file = args.key_file or os.environ.get("EFFECTOR485_KEY_FILE")
                chunks = from_effector485(args.effector485.rstrip("/"), args.agent, Path(key_file).read_text().strip() if key_file else None, typed)
            else:
                chunks = from_effector(args.server.rstrip("/"), args.effector, os.environ["EFFECTOR_KEY"], args.lease,
                                       os.environ.get("EFFECTOR_SERIAL_PIN"), typed)
            typist = Typist(lambda text: typed[0](text)) if args.type else None
            run(stage, chunks, typist, args.timeout, save)
    except (OSError, ValueError, KeyError) as error:
        print(f"board_stage1: the console from {label} could not be read: {error}", file=sys.stderr)
        sys.exit(2)
    finally:
        if save:
            save.close()
    text, missing = stage.report(label)
    print(text)
    sys.exit(1 if missing else 0)


if __name__ == "__main__":
    main()
