#!/usr/bin/env python3
"""One release to several machines, one at a time (801-LNK-0045, 801's S8): each machine gets it through the Effector
server's `update_agent` (751), or over the RS-485 cable (scripts/link_update.py) when the server cannot reach it and
`--cable` names it; the next machine is touched only once this one's agent is back on the server running the release.
A machine that falls back, refuses the release or does not come back within the bound stops the rollout, with the
reason and its log lines, and the machines after it are not touched.

Usage:
    rollout.py (--root DIR | --bundle FILE) MACHINE... [--version N] [--arch x86_64|aarch64] [--wait S] [--poll S]
               [--cable MACHINE]... [--port PORT] [--mindlink PATH] [--mindlink-option OPTION]...
               [--server URL] [--key-file FILE] [--reason TEXT] [--run-id ID] [--commit HEX] [--dry-run]

    MACHINE        an Effector agent ID (mind_353abc18de2dbd0e), in the order the machines are updated
    --root DIR     a signed build (usb_root): made release N, as link_update.py makes one, and its bundle
    --version N    the release's version with --root (default: the time as YYYYMMDDHHMM)
    --bundle FILE  a bundle `release.py bundle` wrote; its channel names the version
    --wait S       how long a machine has from the release's sending to its agent's return on it (default 1200)
    --poll S       how often the server is asked (default 10)
    --cable M      M is the machine on the cable: used when the server cannot reach M or M's agent did not take
                   update_agent (its path is off), once M's log over the cable names M as its agent
    --port PORT    the cable's port, as link_update.py takes it (default COM22)
    --mindlink P   the program link_update.py calls; with effector485 holding the port, an adapter to its daemon
    --server URL   the Effector server (default https://hesere.uz/effector)
    --key-file F   the operator's API key (default ~/.config/mind-effector/hesere/api_key): sent in one header, never
                   printed
    --commit HEX   the commit the release was built from: the machine's `[INIT] BUILD` line must name it
    --run-id ID    makes the operations' idempotency keys (default: the start time); the same ID resumes a run
    --dry-run      reads only: the release is made locally, the server and the cable are read, nothing is sent

A machine is back when the server shows its agent online and taking operations after its `update_agent` answered
`completed`, `updated N, slot S confirmed` (or, over the cable, after its updater logged `SLOT S CONFIRMED ON THE
DISK`), and this boot's log (`collect_logs` of the ring), when the server can collect it, names release N in
`[UPDATER] SLOT S, VERSION N`. It fell back when the answer is `failed` (`fell-back`, `not-confirmed`, the updater's
refusal), when that log names another version, or when the updater refused or rolled back over the cable. An answer
`denied` (or `no-policy`, `expired`, …) came before a byte reached the updater: the machine is as it was, and the
cable is tried when `--cable` names it.

Exit status: 0 when every machine came back on the release, 1 when the rollout stopped, 2 for a usage error or a
release that does not verify.
"""
import argparse
import datetime
import hashlib
import json
import re
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.parse
import urllib.request
import zipfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import link_update  # noqa: E402
import release  # noqa: E402

SERVER = "https://hesere.uz/effector"
KEY_FILE = Path.home() / ".config/mind-effector/hesere/api_key"
REASON = "LNK 801-LNK-0045: a rollout"
PROGRAM = "MIND Core"  # the server's program whose logs a MIND machine's are
ARCH = {"x86_64": "x64", "aarch64": "arm64"}  # the endpoint's architecture for a release's
# Answers that come before a byte reaches the updater: the machine is as it was (751-EFF-0023, contract 1.4).
NOT_TAKEN = {"denied", "no-policy", "unsupported", "action_not_allowed", "command_expired"}
FINAL = {"completed", "failed", "timeout", "expired", "cancelled", "rollback", "denied", "run_as_unavailable",
         "interactive_session_unavailable"}
UPDATED = re.compile(r"updated (\d+), slot ([AB]) confirmed")
AGENT = re.compile(r"EFFECTOR: (?:AGENT (\S+) TO |NEW AGENT ID (\S+))")
BUILD = re.compile(r"\[INIT\] BUILD: BRANCH (\S+), COMMIT (\S+)")
RUNNING = re.compile(r"\[UPDATER\] SLOT ([AB])(?: ON TRIAL)?, VERSION (\d+)")
CONFIRMED = re.compile(r"\[UPDATER\] SLOT [AB] CONFIRMED ON THE DISK|\[INIT\] TRIAL BOOT CONFIRMED")
SHOWN = re.compile(r"\[UPDATER\]|\[INIT\] (BUILD|TRIAL|THE EFFECTOR AGENT|THE REFUSED)|"
                   r"EFFECTOR: .*(UPDATE|RELEASE|STAGED|fell back|updated |not staged|THE SERVER)")
SHOWN_MAX = 40  # log lines shown with a stop
LOG_WAIT = 150  # seconds a collect_logs may take


class Stop(Exception):
    """The rollout stops at this machine: why, and the log lines that show it."""
    def __init__(self, why, lines=()):
        super().__init__(why)
        self.why, self.lines = why, list(lines)


class ServerError(Exception):
    pass


class NoRedirect(urllib.request.HTTPRedirectHandler):
    """A redirect is an error: the key would follow it (docs/effector/protocol.md)."""
    def redirect_request(self, *args, **kwargs):
        return None


class Server:
    """The Effector server's operator API. The key goes into the Authorization header and nowhere else."""
    def __init__(self, base, key, timeout=60):
        url = urllib.parse.urlsplit(base)
        if not (url.scheme == "https" or (url.scheme == "http" and url.hostname in ("127.0.0.1", "localhost", "::1"))):
            raise ValueError(f"{base}: the key goes over HTTPS only, or to this machine's loopback")
        self.base, self.__key, self.timeout = base.rstrip("/"), key, timeout
        self.opener = urllib.request.build_opener(NoRedirect)

    def call(self, method, path, body=None, data=None, raw=False, timeout=None):
        headers = {"Authorization": f"Bearer {self.__key}", "Accept": "application/json"}
        if body is not None:
            data, headers["Content-Type"] = json.dumps(body).encode(), "application/json"
        elif data is not None:
            headers["Content-Type"] = "application/octet-stream"
        request = urllib.request.Request(self.base + path, data=data, method=method, headers=headers)
        try:
            with self.opener.open(request, timeout=timeout or self.timeout) as r:
                text = r.read()
        except urllib.error.HTTPError as e:
            detail = e.read()[:300].decode(errors="replace").strip()
            raise ServerError(f"{method} {path.split('?')[0]}: HTTP {e.code} {detail}", e.code) from None
        except (urllib.error.URLError, OSError, ValueError) as e:
            raise ServerError(f"{method} {path.split('?')[0]}: {getattr(e, 'reason', e)}", None) from None
        if raw:
            return text.decode(errors="replace")
        try:
            return json.loads(text) if text.strip() else {}
        except ValueError:
            raise ServerError(f"{method} {path.split('?')[0]}: not JSON", None) from None

    def endpoint(self, agent):
        """The endpoint as the server shows it, or None when it knows no such agent."""
        try:
            return self.call("GET", f"/api/v1/endpoints?id={urllib.parse.quote(agent)}")
        except ServerError as e:
            if e.args[1] == 404:
                return None
            raise

    def upload(self, path, name, sha256):
        q = urllib.parse.urlencode({"name": name, "sha256": sha256})
        return self.call("POST", f"/api/v1/files?{q}", data=Path(path).read_bytes(), timeout=900)

    def operation(self, body):
        return self.call("POST", "/api/v1/operations", body)

    def get_operation(self, op_id):
        return self.call("GET", f"/api/v1/operations?id={urllib.parse.quote(op_id)}")

    def artifact(self, artifact_id):
        return self.call("GET", f"/api/v1/artifacts/{urllib.parse.quote(artifact_id)}", raw=True)


class Release:
    """One release: its version and architecture, the server tree link_update.py puts over the cable, and the bundle
    update_agent sends."""
    def __init__(self, version, arch, server, bundle):
        self.version, self.arch, self.server, self.bundle = int(version), arch, Path(server), Path(bundle)
        self.manifest = (self.server / f"releases/{self.version}/{arch}/MANIFEST").read_bytes()
        data = self.bundle.read_bytes()
        self.size, self.sha256 = len(data), hashlib.sha256(data).hexdigest()


def from_root(root, arch, version, temp):
    """Release `version` of the signed build `root`, made as link_update.py makes one, and its bundle (751-UPD-0032)."""
    server, _ = link_update.build(root, arch, version, temp)
    out = Path(temp) / f"mind-release-{version}-{arch}.zip"
    release.bundle(server, out, arches=[arch])
    return Release(version, arch, server, out)


def from_bundle(path, arch, temp):
    """The release a bundle holds: unpacked into a server tree under `temp` and checked as release.py checks one."""
    server = Path(temp) / "server"
    with zipfile.ZipFile(path) as z:
        for name in z.namelist():
            if name.startswith("/") or "\\" in name or ".." in Path(name).parts:
                raise release.ReleaseError(f"{path}: {name!r} is not a path of the layout")
        z.extractall(server)
    whole = (server / "channels/stable").read_bytes() if (server / "channels/stable").is_file() else b""
    c, why = release.channel_from(whole.split(b"\n")[0] + b"\n", "stable")
    if c is None:
        raise release.ReleaseError(f"{path}: channels/stable: {why}")
    reason = release.check(server)
    if reason:
        raise release.ReleaseError(f"{path}: {reason}")
    arches = sorted(c["manifests"])
    if arch is None and len(arches) != 1:
        raise release.ReleaseError(f"{path} holds {', '.join(arches)}: name one with --arch")
    arch = arch or arches[0]
    if arch not in arches:
        raise release.ReleaseError(f"{path} holds no {arch} manifest")
    return Release(c["version"], arch, server, path)


def shown(text):
    """The lines of a log that say how an update went, the last SHOWN_MAX."""
    return [line for line in text.splitlines() if SHOWN.search(line)][-SHOWN_MAX:]


def ready(e):
    """The server reaches the agent now: online, activated and polling (the server's `operations_ready`)."""
    return bool(e and e.get("online") and e.get("operations_ready"))


def describe(e):
    name = e.get("address_alias") or e.get("hostname") or "?"
    reach = "taking operations" if ready(e) else "online, not taking operations" if e.get("online") else "offline"
    state = "" if e.get("state") in (None, "online", "offline") else f" ({e['state']})"
    return f"{name}, {reach}{state}, last seen {e.get('last_seen') or 'never'}"


class Rollout:
    def __init__(self, server, rel, machines, cable=(), link=None, wait=1200, poll=10, reason=REASON, run_id="run",
                 commit=None, dry_run=False, say=print, clock=time.monotonic, sleep=time.sleep):
        self.server, self.rel, self.machines, self.cable, self.link = server, rel, list(machines), set(cable), link
        self.wait, self.poll, self.reason, self.run_id, self.commit = wait, poll, reason, run_id, commit
        self.dry_run, self.say, self.clock, self.sleep = dry_run, say, clock, sleep
        self.artifact, self.logs = None, 0

    def key(self, agent, what):
        return f"rollout-{self.rel.version}-{self.run_id}-{agent}-{what}"

    def note(self, agent, what):
        i = self.machines.index(agent) + 1
        return f"{self.reason}: {what}, release {self.rel.version}, {agent} ({i} of {len(self.machines)})"[:1024]

    def run(self):
        rel, n = self.rel, len(self.machines)
        self.say(f"RELEASE {rel.version} ({rel.arch}): BUNDLE {rel.size} BYTES, SHA-256 {rel.sha256}")
        self.say(f"{n} MACHINES, ONE AT A TIME, EACH GIVEN {self.wait:g} S TO COME BACK"
                 + (" (A DRY RUN: NOTHING IS SENT)" if self.dry_run else ""))
        problems = self.preflight()
        if problems:
            for p in problems:
                self.say(f"rollout: {p}")
            self.say("NOTHING WAS SENT")
            return 2
        for i, agent in enumerate(self.machines, 1):
            try:
                self.one(i, agent)
            except Stop as s:
                self.say(f"{'WOULD STOP' if self.dry_run else 'STOPPED'} AT {agent} ({i} OF {n}): {s.why}")
                if s.lines:
                    self.say("ITS LOG:")
                for line in s.lines[-2 * SHOWN_MAX:]:
                    self.say(f"    {line}")
                if self.machines[i:]:
                    self.say(f"NOT TOUCHED: {', '.join(self.machines[i:])}")
                return 1
        self.say(f"{'THE PLAN HOLDS' if self.dry_run else 'DONE'}: RELEASE {rel.version} ON ALL {n} MACHINES")
        return 0

    def preflight(self):
        """What would stop the rollout before it starts: a machine twice or unknown, of another processor, or a cable
        machine not in the list. Nothing is sent before these are answered."""
        problems = [f"{m} is named twice" for m in sorted({m for m in self.machines if self.machines.count(m) > 1})]
        problems += [f"--cable {m} is not one of the machines" for m in sorted(self.cable - set(self.machines))]
        if self.cable and self.link is None:
            problems.append("--cable needs the link (--mindlink)")
        for agent in dict.fromkeys(self.machines):
            try:
                e = self.server.endpoint(agent)
            except ServerError as err:
                problems.append(f"{agent}: the server could not be read: {err.args[0]}")
                continue
            if e is None:
                problems.append(f"{agent}: the server knows no such machine")
            elif e.get("architecture") and e["architecture"] != ARCH[self.rel.arch]:
                problems.append(f"{agent} is {e['architecture']}, the release {self.rel.arch}")
        return problems

    def one(self, i, agent):
        e = self.endpoint(agent)
        self.say(f"MACHINE {i} OF {len(self.machines)}: {agent} ({describe(e)})")
        if ready(e):
            if self.dry_run:
                self.say("  WOULD SEND update_agent THROUGH THE SERVER, THEN WAIT FOR ITS AGENT ON THE RELEASE")
                return
            why = self.through_server(agent)
            if why is None:
                return
        else:
            why = "the server cannot reach its agent now"
        if agent not in self.cable:
            raise Stop(f"{why}, and it is not the machine on the cable (--cable)", self.server_lines(agent))
        self.over_cable(agent, why)

    def endpoint(self, agent):
        try:
            e = self.server.endpoint(agent)
        except ServerError as err:
            raise Stop(f"the server could not be read: {err.args[0]}") from None
        if e is None:
            raise Stop("the server knows no such machine")
        return e

    def through_server(self, agent):
        """The release through update_agent, followed to the agent's return; the reason it was not taken (nothing
        reached the updater), or a Stop."""
        rel = self.rel
        if self.artifact is None:
            try:
                item = self.server.upload(rel.bundle, f"mind-release-{rel.version}-{rel.arch}.zip", rel.sha256)
            except ServerError as err:
                return f"the server did not take the bundle ({err.args[0]}): nothing was sent to the machine"
            if item.get("sha256") != rel.sha256 or item.get("size") != rel.size or not item.get("id"):
                raise Stop(f"the server holds the bundle as {item.get('size')} bytes, {item.get('sha256')}")
            self.artifact = item["id"]
            self.say(f"  THE BUNDLE UPLOADED: ARTIFACT {self.artifact}")
        seconds = max(60, min(3600, int(self.wait)))
        body = {"agent_id": agent, "kind": "update_agent", "artifact_id": self.artifact, "version": rel.version,
                "timeout_seconds": seconds, "queue_ttl_seconds": seconds, "idempotency_key": self.key(agent, "update"),
                "reason": self.note(agent, "update_agent")}
        try:
            op = self.post(body)
        except ServerError as err:
            return f"the server refused update_agent ({err.args[0]}): nothing was sent to the machine"
        self.say(f"  SENT: update_agent, OPERATION {op}")
        return self.follow(agent, op, self.clock() + self.wait)

    def post(self, body):
        """An operation created; a failed transport or a 5xx is tried again with the same idempotency key."""
        for attempt in range(3):
            try:
                op = self.server.operation(body)
            except ServerError as err:
                if err.args[1] is not None and err.args[1] < 500 or attempt == 2:
                    raise
                self.sleep(self.poll)
                continue
            if not op.get("id"):
                raise ServerError(f"the server answered {body['kind']} without an operation", None)
            return op["id"]

    def follow(self, agent, op_id, deadline):
        last, seen, status = None, None, "not read"
        while True:
            try:
                op = self.server.get_operation(op_id)
            except ServerError as err:
                op = None
                if last != err.args[0]:
                    self.say(f"  THE SERVER: {err.args[0]}")
                    last = err.args[0]
            if op is not None:
                status, error = op.get("status") or "?", op.get("error") or (op.get("result") or {}).get("error") or ""
                late = op.get("late_acknowledgement") or {}
                if status == "timeout" and late.get("status"):
                    status, error = late["status"], late.get("error") or ""
                text = f"{status}{': ' + error if error else ''}{' (a late answer, without its text)' if late else ''}"
                if text != last:
                    self.say(f"  OPERATION {op_id}: {text}")
                    last = text
                if status == "completed":
                    return self.came_back(agent, op, deadline)
                if status in FINAL and status != "timeout":
                    if status in ("denied", "expired") or error in NOT_TAKEN:
                        return f"its agent did not take update_agent ({text}): nothing reached its updater"
                    raise Stop(self.refusal(op, status, error), self.server_lines(agent))
            try:
                e = self.server.endpoint(agent)
                state = "taking operations" if ready(e) else "offline" if not (e or {}).get("online") else "online"
                if state != seen:
                    self.say(f"  ITS AGENT: {state}")
                    seen = state
            except ServerError:
                pass
            if self.clock() >= deadline:
                lines = self.cable_lines() if agent in self.cable else self.server_lines(agent)
                raise Stop(f"its agent did not come back on release {self.rel.version} within {self.wait:g} s "
                           f"(the operation: {status}; its agent: {seen or 'not read'})", lines)
            self.sleep(self.poll)

    def refusal(self, op, status, error):
        stdout = ((op.get("result") or {}).get("stdout") or "").strip()
        if error == "fell-back" or stdout.startswith("fell back"):
            return f"it fell back: {stdout.removeprefix('fell back: ') or error}"
        return f"update_agent {status}: {error or '-'}" + (f": {stdout}" if stdout else "")

    def came_back(self, agent, op, deadline):
        """Answered completed: the agent's answer names the release, the server shows it taking operations, and its
        boot log, when it can be read, says the slot runs the release."""
        stdout = ((op.get("result") or {}).get("stdout") or "").strip()
        named = UPDATED.search(stdout)
        if named and int(named.group(1)) != self.rel.version:
            raise Stop(f"it answered {stdout!r}: not release {self.rel.version}", self.server_lines(agent))
        self.back(agent, deadline, stdout or "update_agent completed, its answer without its text", proved=bool(named))

    def back(self, agent, deadline, said, proved):
        """The agent taking operations again, and this boot's log checked: a Stop when it is not back in time, the log
        names another version or commit, or nothing proves the version (`proved`: the answer or the cable named it)."""
        while True:
            try:
                if ready(self.server.endpoint(agent)):
                    break
            except ServerError:
                pass
            if self.clock() >= deadline:
                raise Stop(f"{said}, but its agent is not back on the server within {self.wait:g} s",
                           self.cable_lines() if agent in self.cable else [])
            self.sleep(self.poll)
        # The ring is this boot's; a saved boot log may still be the boot before's in a boot's first minutes.
        log = self.collect(agent, "ring")
        running = RUNNING.findall(log or "")
        if running and int(running[0][1]) != self.rel.version:
            raise Stop(f"its log says slot {running[0][0]} runs version {running[0][1]}, not "
                       f"{self.rel.version}: it fell back", shown(log))
        build = BUILD.search(log or "")
        if self.commit and build and not build.group(2).lower().startswith(self.commit.lower()):
            raise Stop(f"its log names commit {build.group(2)}, not {self.commit}", shown(log))
        if not proved and not running:
            raise Stop("its agent is back, but nothing shows it runs release "
                       f"{self.rel.version}: the answer came without its text and its log names no version")
        self.say(f"  BACK ON RELEASE {self.rel.version}: {said}")
        evidence = [line for line in (log or "").splitlines() if any(r.search(line) for r in (BUILD, RUNNING, CONFIRMED))]
        for line in evidence[:4]:
            self.say(f"    {line.strip()}")

    def collect(self, agent, name):
        """A log through the server's collect_logs (`ring`, this boot's; `boot`, the newest saved), or None (the policy,
        the time); never in a dry run."""
        if self.dry_run:
            return None
        self.logs += 1
        body = {"agent_id": agent, "kind": "collect_logs", "program_name": PROGRAM, "name": name,
                "timeout_seconds": 120, "idempotency_key": self.key(agent, f"log{self.logs}"),
                "reason": self.note(agent, f"its log ({name})")}
        try:
            op_id = self.server.operation(body)["id"]
            until = self.clock() + LOG_WAIT
            while True:
                op = self.server.get_operation(op_id)
                if op.get("status") in FINAL:
                    break
                if self.clock() >= until:
                    self.say(f"  ITS LOG ({name}): NOT COLLECTED IN TIME")
                    return None
                self.sleep(self.poll)
            result = op.get("result") or {}
            artifact = (result.get("artifact") or {}).get("id") or result.get("artifact_id")
            if op.get("status") != "completed" or not artifact:
                why = op.get("error") or result.get("error") or "-"
                self.say(f"  ITS LOG ({name}): NOT COLLECTED ({op.get('status')}: {why})")
                return None
            return self.server.artifact(artifact)
        except (ServerError, KeyError, TypeError) as err:
            self.say(f"  ITS LOG ({name}): NOT COLLECTED ({err.args[0] if err.args else err})")
            return None

    def server_lines(self, agent):
        """A stop's lines: this boot's (after a fallback, its updater names the refused trial and why) and the newest
        saved boot log's (in a boot's first minutes, the refused trial's own); the cable's when the server has none."""
        try:
            e = self.server.endpoint(agent)
        except ServerError:
            e = None
        lines, seen = [], []
        if ready(e):
            for name, label in (("ring", "this boot"), ("boot", "the newest saved boot log")):
                log = self.collect(agent, name)
                if log and shown(log) and shown(log) != seen:
                    seen = shown(log)
                    lines += [f"--- {label}:"] + seen
        return lines or (self.cable_lines() if agent in self.cable else [])

    def cable_lines(self):
        try:
            text = self.link.log() if self.link is not None else None
        except (RuntimeError, OSError, subprocess.TimeoutExpired):
            text = None
        return shown(text) if text else []

    def over_cable(self, agent, why):
        """The release over the cable to the machine its log names as `agent`, then its agent awaited on the server."""
        self.say(f"  {why}: OVER THE CABLE")
        try:
            text = self.link.log()
        except (RuntimeError, OSError, subprocess.TimeoutExpired) as err:
            raise Stop(f"{why}, and the cable's machine could not be read: {err}") from None
        if text is None:
            raise Stop(f"{why}, and the cable's machine does not answer")
        names = [m.group(1) or m.group(2) for m in AGENT.finditer(text)]
        if not names:
            raise Stop(f"{why}, and the log over the cable names no Effector agent (its ring may have dropped the "
                       "agent's start line): the machine on the cable is not confirmed, nothing sent", shown(text))
        if names[-1] != agent:
            raise Stop(f"{why}, and the cable reaches {names[-1]}, not {agent}: nothing sent")
        self.say(f"  THE CABLE REACHES {agent}: ITS LOG NAMES IT")
        if self.dry_run:
            self.say("  WOULD PUT THE RELEASE OVER THE CABLE, FOLLOW ITS UPDATER, THEN WAIT FOR ITS AGENT ON THE SERVER")
            return
        rel, sent = self.rel, []
        try:
            line = link_update.update(self.link, rel.server, rel.manifest, rel.arch, rel.version, self.wait,
                                      say=lambda s: self.say(f"    {s}"), sent=lambda: sent.append(self.clock()))
        except (RuntimeError, subprocess.TimeoutExpired, release.ReleaseError) as err:
            raise Stop(f"over the cable: {err}", self.cable_lines()) from None
        # A machine confirmed at the bound still gets a minute to reach the server.
        deadline = max((sent[0] if sent else self.clock()) + self.wait, self.clock() + 60)
        self.back(agent, deadline, f"over the cable: {line.strip()}", proved=True)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("machines", nargs="+", metavar="MACHINE")
    given = parser.add_mutually_exclusive_group(required=True)
    given.add_argument("--root", type=Path)
    given.add_argument("--bundle", type=Path)
    parser.add_argument("--version", type=int)
    parser.add_argument("--arch", choices=sorted(release.ARCHES))
    parser.add_argument("--wait", type=float, default=1200)
    parser.add_argument("--poll", type=float, default=10)
    parser.add_argument("--cable", action="append", default=[])
    parser.add_argument("--port", default="COM22")
    parser.add_argument("--mindlink", default="mindlink")
    parser.add_argument("--mindlink-option", action="append", default=[])
    parser.add_argument("--server", default=SERVER)
    parser.add_argument("--key-file", type=Path, default=KEY_FILE)
    parser.add_argument("--reason", default=REASON)
    parser.add_argument("--run-id", default=datetime.datetime.now(datetime.timezone.utc).strftime("%Y%m%dT%H%M%S"))
    parser.add_argument("--commit")
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args(argv)
    if not re.fullmatch(r"[A-Za-z0-9_.-]{1,64}", args.run_id) or args.wait <= 0 or args.poll < 0:
        print("rollout: --run-id is 1-64 of [A-Za-z0-9_.-]; --wait above 0; --poll not below 0", file=sys.stderr)
        return 2
    try:
        key = args.key_file.read_text().strip()
    except OSError as e:
        print(f"rollout: the key file {args.key_file}: {e.strerror}", file=sys.stderr)
        return 2
    if not key:
        print(f"rollout: the key file {args.key_file} is empty", file=sys.stderr)
        return 2

    def say(text):
        print(str(text).replace(key, "[the key]"), flush=True)

    try:
        server = Server(args.server, key)
    except ValueError as e:
        print(f"rollout: {e}", file=sys.stderr)
        return 2
    with tempfile.TemporaryDirectory(prefix="mind-rollout-") as temp:
        try:
            if args.root:
                version = args.version or int(datetime.datetime.now(datetime.timezone.utc).strftime("%Y%m%d%H%M"))
                rel = from_root(args.root, args.arch or "x86_64", version, temp)
            else:
                rel = from_bundle(args.bundle, args.arch, temp)
                if args.version not in (None, rel.version):
                    raise release.ReleaseError(f"{args.bundle} holds release {rel.version}, not {args.version}")
        except (release.ReleaseError, OSError, ValueError, zipfile.BadZipFile) as e:
            say(f"rollout: the release: {e}")
            return 2
        link = link_update.Link(args.mindlink, args.port, args.mindlink_option) if args.cable else None
        rollout = Rollout(server, rel, args.machines, cable=args.cable, link=link, wait=args.wait, poll=args.poll,
                          reason=args.reason, run_id=args.run_id, commit=args.commit, dry_run=args.dry_run, say=say)
        try:
            return rollout.run()
        except KeyboardInterrupt:
            say("INTERRUPTED: the machine being updated may still be on its way; read its operation on the server")
            return 1


if __name__ == "__main__":
    sys.exit(main())
