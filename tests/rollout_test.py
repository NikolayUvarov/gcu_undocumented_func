#!/usr/bin/env python3
"""Host test of scripts/rollout.py (801-LNK-0045): a release to machines one at a time against a stand-in Effector
server on 127.0.0.1 (its operator API as hesere answers it: endpoints, files, operations, artifacts), and a scripted
cable. Every machine updated in order, each only after the one before came back; a fallback stops the rollout before
the next machine; so does a machine that never comes back; a machine the server cannot update goes over the cable only
once its log names it; a dry run sends nothing; the API key never appears in the output. No real server or machine."""
import contextlib
import hashlib
import io
import json
import os
import sys
import tempfile
import threading
import unittest
import urllib.parse
import zipfile
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
import rollout  # noqa: E402
import sign_manifest as sm  # noqa: E402

KEY = "efk_rollout-test-0123456789abcdef-never-printed"
VERSION = 202611141530


def ring(agent, version, slot="B", refused=False):
    """A ring as collect_logs gives it: this boot's updater, its build and its agent."""
    lines = ["[    0.287] init(1) [INIT] BUILD: BRANCH HEAD, COMMIT 0123456789ab",
             f"[    3.495] updater(5) [UPDATER] SLOT {slot}{' ON TRIAL' if slot == 'B' else ''}, VERSION {version}; THE NEWER RECORD: SLOT {slot}",
             f"[    4.289] effector(9) EFFECTOR: AGENT {agent} TO hesere.uz/effector (0 SAVED RESULTS, 0 TO ACKNOWLEDGE)"]
    if refused:
        lines.append(f"[    3.496] updater(5) [UPDATER] A TRIAL REFUSED BEFORE (system/updater/refused): refused version "
                     f"{VERSION} in slot B, falling back to slot A; why no-network: no address on any card")
    else:
        lines += ["[    9.000] init(1) [INIT] TRIAL BOOT CONFIRMED: THE EFFECTOR SERVER ACCEPTED THE AGENT'S HEARTBEAT 3 S AFTER READY",
                  "[    9.001] updater(5) [UPDATER] SLOT B CONFIRMED ON THE DISK: INIT CONFIRMED THE TRIAL BOOT"]
    return "\n".join(lines) + "\n"


class Bench:
    """The stand-in server's state: each machine's behaviour for update_agent, what it runs, and every request (method
    and path, never a header or a body)."""
    def __init__(self, machines):
        self.machines = {a: {"how": how, "online": how != "offline", "version": 5, "refused": False}
                         for a, how in machines.items()}
        self.requests, self.events, self.ops, self.keys, self.files = [], [], {}, {}, {}
        self.lock = threading.Lock()

    def endpoint(self, agent):
        m = self.machines[agent]
        return {"id": agent, "hostname": f"host-{agent}", "address_alias": f"alias-{agent}", "platform": "mindcore",
                "architecture": "x64", "online": m["online"], "state": "online" if m["online"] else "offline",
                "operations_ready": m["online"], "activated": True, "last_seen": "2026-11-14T15:30:00Z"}

    def advance(self, op):
        """The update's operation one poll further, as the machine's behaviour has it."""
        m, polls = self.machines[op["agent_id"]], op["polls"]
        op["polls"] += 1
        how = m["how"]
        if how == "denied":
            return self.final(op, "failed", "denied", "")
        if polls < 1:
            return {"status": "queued"}
        if polls < 2 or (polls < 4 and how in ("ok", "late", "fallback")):
            m["online"] = polls < 2  # the restart: offline while the trial boots
            return {"status": "running"}
        if how == "never":
            m["online"] = False
            return {"status": "running"}
        m["online"] = True
        if how == "fallback":
            m["refused"] = True
            return self.final(op, "failed", "fell-back", f"fell back: slot A runs version 5, not {op['version']}; "
                              f"the updater: version {op['version']} in slot B: no-network: no address on any card")
        m["version"] = op["version"]
        if how == "late":
            done = self.final(op, "timeout", "agent did not acknowledge the operation before timeout_seconds elapsed", "")
            done["late_acknowledgement"] = {"status": "completed", "exit_code": 0, "delay_seconds": 12.5}
            return done
        return self.final(op, "completed", "", f"updated {op['version']}, slot B confirmed")

    def final(self, op, status, error, stdout):
        if not op.get("final"):
            op["final"] = True
            self.events.append((status, op["agent_id"]))
        result = {"status": status, "exit_code": 0 if status == "completed" else 1, "stdout": stdout, "run_as": "effector"}
        if error:
            result["error"] = error
        return {"status": status, "result": result, **({"error": error} if error else {})}


def handler(bench):
    class Handler(BaseHTTPRequestHandler):
        def log_message(self, *args):
            pass

        def answer(self, code, body, raw=False):
            data = body.encode() if raw else json.dumps(body).encode()
            self.send_response(code)
            self.send_header("Content-Type", "text/plain" if raw else "application/json")
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            self.wfile.write(data)

        def route(self, method):
            url = urllib.parse.urlsplit(self.path)
            q = dict(urllib.parse.parse_qsl(url.query))
            data = self.rfile.read(int(self.headers.get("Content-Length") or 0)) if method == "POST" else b""
            with bench.lock:
                bench.requests.append((method, url.path))
                if self.headers.get("Authorization") != f"Bearer {KEY}":
                    return self.answer(401, {"error": "invalid API key"})
                if method == "GET" and url.path == "/effector/api/v1/endpoints":
                    if q.get("id") not in bench.machines:
                        return self.answer(404, {"error": "endpoint not found"})
                    return self.answer(200, bench.endpoint(q["id"]))
                if method == "POST" and url.path == "/effector/api/v1/files":
                    if hashlib.sha256(data).hexdigest() != q.get("sha256"):
                        return self.answer(400, {"error": "sha256 mismatch"})
                    item = {"id": f"upload-{len(bench.files) + 1}", "size": len(data), "sha256": q["sha256"], "name": q.get("name")}
                    bench.files[item["id"]] = data
                    return self.answer(201, item)
                if method == "POST" and url.path == "/effector/api/v1/operations":
                    body = json.loads(data)
                    if not body.get("idempotency_key") or not body.get("reason") or body.get("agent_id") not in bench.machines:
                        return self.answer(400, {"error": "agent_id, idempotency_key, and a reason are required"})
                    if body["idempotency_key"] in bench.keys:
                        return self.answer(200, {"id": bench.keys[body["idempotency_key"]]})
                    if body["kind"] == "update_agent":
                        if body.get("artifact_id") not in bench.files or not isinstance(body.get("version"), int) or not 1 <= body.get("timeout_seconds", 0) <= 3600:
                            return self.answer(400, {"error": "artifact_id is required for a MIND system update"})
                        bench.events.append(("update_agent", body["agent_id"]))
                    elif body["kind"] != "collect_logs" or body.get("program_name") != "MIND Core":
                        return self.answer(400, {"error": "unknown kind"})
                    op_id = f"op-{len(bench.ops) + 1}"
                    bench.ops[op_id] = {"id": op_id, "polls": 0, **body}
                    bench.keys[body["idempotency_key"]] = op_id
                    return self.answer(201, {"id": op_id, "status": "queued"})
                if method == "GET" and url.path == "/effector/api/v1/operations" and q.get("id") in bench.ops:
                    op = bench.ops[q["id"]]
                    if op["kind"] == "collect_logs":
                        m = bench.machines[op["agent_id"]]
                        state = {"status": "completed", "result": {"status": "completed", "exit_code": 0, "artifact": {"id": f"art-{op['id']}"}}}
                        op["log"] = ring(op["agent_id"], m["version"], "A" if m["refused"] else "B", m["refused"])
                    else:
                        state = bench.advance(op)
                    return self.answer(200, {"id": op["id"], "agent_id": op["agent_id"], "kind": op["kind"], **state})
                if method == "GET" and url.path.startswith("/effector/api/v1/artifacts/"):
                    op = bench.ops.get(url.path.rsplit("/", 1)[1].removeprefix("art-"))
                    return self.answer(200, op["log"], raw=True) if op and "log" in op else self.answer(404, {"error": "no artifact"})
                return self.answer(404, {"error": "no route"})

        def do_GET(self):
            self.route("GET")

        def do_POST(self):
            self.route("POST")
    return Handler


class Cable:
    """A scripted link (link_update.Link's put, mkdir, log): the ring names `named`; once the channel's timestamp is
    put, the updater confirms the new slot and the bench's machine comes back on the release."""
    def __init__(self, bench, agent, named=None):
        self.bench, self.agent, self.named, self.puts, self.after = bench, agent, named or agent, [], ""

    def put(self, path, name):
        assert Path(path).is_file(), path
        self.puts.append(name)
        if name == "timestamps/stable":
            self.after = (f"20.000 updater: [UPDATER] SLOT B ON TRIAL, VERSION {VERSION}\n"
                          "21.000 updater: [UPDATER] SLOT B CONFIRMED ON THE DISK: INIT CONFIRMED THE TRIAL BOOT\n")
            with self.bench.lock:
                m = self.bench.machines[self.agent]
                m.update(online=True, version=VERSION)

    def log(self):
        return (f"1.000 init: [INIT] BUILD: BRANCH HEAD, COMMIT 0123456789ab\n"
                f"4.289 effector: EFFECTOR: AGENT {self.named} TO hesere.uz/effector (0 SAVED RESULTS, 0 TO ACKNOWLEDGE)\n"
                + self.after)


def volume(where):
    v = Path(where)
    (v / "EFI/BOOT").mkdir(parents=True)
    (v / "EFI/BOOT/BOOTX64.EFI").write_bytes(b"MZ loader")
    (v / "kernel.elf").write_bytes(b"\x7fELF kernel")
    (v / "shell.elf").write_bytes(b"\x7fELF shell")
    sm.sign_volume(v, sm.TEST_SEED, ("c" * 40, "nightly-x", "d" * 64), version=5)
    return v


class Rollouts(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp = tempfile.TemporaryDirectory(prefix="rollout-test-")
        cls.dir = Path(cls.temp.name)
        cls.root = volume(cls.dir / "usb_root")
        cls.key_file = cls.dir / "api_key"
        cls.key_file.write_text(KEY + "\n")
        cls.release = rollout.from_root(cls.root, "x86_64", VERSION, cls.dir / "release")

    @classmethod
    def tearDownClass(cls):
        cls.temp.cleanup()

    def serve(self, machines):
        bench = Bench(machines)
        server = ThreadingHTTPServer(("127.0.0.1", 0), handler(bench))
        threading.Thread(target=server.serve_forever, daemon=True).start()
        self.addCleanup(server.server_close)
        self.addCleanup(server.shutdown)
        return bench, f"http://127.0.0.1:{server.server_address[1]}/effector"

    def main(self, url, *args, key_file=None):
        out = io.StringIO()
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(out):
            code = rollout.main(["--root", str(self.root), "--version", str(VERSION), "--server", url,
                                 "--key-file", str(key_file or self.key_file), "--poll", "0.01", "--run-id", "t", *args])
        text = out.getvalue()
        self.assertNotIn(KEY, text)  # the key is never printed, whatever the run
        return code, text

    def rollout(self, bench, url, machines, link, **kw):
        said = []
        r = rollout.Rollout(rollout.Server(url, KEY), self.release, machines, cable=[machines[0]], link=link,
                            wait=kw.pop("wait", 5), poll=0.01, run_id="c", say=said.append, **kw)
        code = r.run()
        text = "\n".join(said)
        self.assertNotIn(KEY, text)
        return code, text

    def updates(self, bench):
        return [a for kind, a in bench.events if kind == "update_agent"]

    def test_every_machine_updated_in_order(self):
        bench, url = self.serve({"mind_a": "ok", "mind_b": "late", "mind_c": "ok"})
        code, text = self.main(url, "mind_a", "mind_b", "mind_c")
        self.assertEqual(code, 0, text)
        self.assertEqual(self.updates(bench), ["mind_a", "mind_b", "mind_c"])
        # Each machine was sent the release only once the one before had answered.
        self.assertEqual(bench.events, [("update_agent", "mind_a"), ("completed", "mind_a"), ("update_agent", "mind_b"),
                                        ("timeout", "mind_b"), ("update_agent", "mind_c"), ("completed", "mind_c")])
        self.assertEqual(len(bench.files), 1, "one upload of the bundle for every machine")
        with zipfile.ZipFile(io.BytesIO(bench.files["upload-1"])) as z:  # the bundle release.py writes, of this release
            self.assertEqual(json.loads(z.read("channels/stable").split(b"\n")[0])["version"], VERSION)
            self.assertIn(f"releases/{VERSION}/x86_64/MANIFEST", z.namelist())
        self.assertIn(f"BACK ON RELEASE {VERSION}: updated {VERSION}, slot B confirmed", text)
        self.assertIn(f"[UPDATER] SLOT B ON TRIAL, VERSION {VERSION}", text)
        self.assertIn(f"DONE: RELEASE {VERSION} ON ALL 3 MACHINES", text)
        # The late answer (no text) was taken on the ring's word that slot B runs the release.
        self.assertIn("mind_b", text)
        sent = [op for op in bench.ops.values() if op["kind"] == "update_agent"]
        self.assertTrue(all(op["version"] == VERSION and op["artifact_id"] == "upload-1" for op in sent), sent)

    def test_a_fallback_stops_before_the_next_machine(self):
        bench, url = self.serve({"mind_a": "ok", "mind_b": "fallback", "mind_c": "ok"})
        code, text = self.main(url, "mind_a", "mind_b", "mind_c")
        self.assertEqual(code, 1, text)
        self.assertEqual(self.updates(bench), ["mind_a", "mind_b"])
        self.assertNotIn("mind_c", [op["agent_id"] for op in bench.ops.values()], "the next machine is not touched at all")
        self.assertIn(f"STOPPED AT mind_b (2 OF 3): it fell back: slot A runs version 5, not {VERSION}; the updater:", text)
        self.assertIn("[UPDATER] A TRIAL REFUSED BEFORE (system/updater/refused)", text)
        self.assertIn("NOT TOUCHED: mind_c", text)

    def test_a_machine_that_never_comes_back_stops_the_rollout(self):
        bench, url = self.serve({"mind_a": "never", "mind_b": "ok"})
        code, text = self.main(url, "mind_a", "mind_b", "--wait", "1")
        self.assertEqual(code, 1, text)
        self.assertEqual(self.updates(bench), ["mind_a"])
        self.assertIn(f"STOPPED AT mind_a (1 OF 2): its agent did not come back on release {VERSION} within 1 s", text)
        self.assertIn("ITS AGENT: offline", text)
        self.assertIn("NOT TOUCHED: mind_b", text)

    def test_offline_and_not_on_the_cable_stops(self):
        bench, url = self.serve({"mind_a": "offline", "mind_b": "ok"})
        code, text = self.main(url, "mind_a", "mind_b")
        self.assertEqual(code, 1, text)
        self.assertEqual(self.updates(bench), [])
        self.assertIn("the server cannot reach its agent now, and it is not the machine on the cable", text)

    def test_the_dry_run_sends_nothing(self):
        bench, url = self.serve({"mind_a": "ok", "mind_b": "offline"})
        calls = self.dir / "mindlink-calls"
        fake = self.dir / "fake-mindlink"
        fake.write_text("#!/bin/sh\n"
                        f"echo \"$*\" >> '{calls}'\n"
                        "[ \"$2\" = log ] && echo '4.289 effector: EFFECTOR: AGENT mind_b TO hesere.uz/effector (0 SAVED RESULTS, 0 TO ACKNOWLEDGE)'\n"
                        "exit 0\n")
        fake.chmod(0o755)
        code, text = self.main(url, "mind_a", "mind_b", "--cable", "mind_b", "--mindlink", str(fake), "--dry-run")
        self.assertEqual(code, 0, text)
        self.assertEqual([m for m, _ in bench.requests], ["GET"] * len(bench.requests), bench.requests)
        self.assertEqual(bench.ops, {})
        self.assertEqual(calls.read_text().split("\n")[:-1], ["COM22 log"], "only the cable's log is read")
        self.assertIn("WOULD SEND update_agent THROUGH THE SERVER", text)
        self.assertIn("THE CABLE REACHES mind_b", text)
        self.assertIn(f"THE PLAN HOLDS: RELEASE {VERSION} ON ALL 2 MACHINES", text)

    def test_unknown_machines_and_a_wrong_key_send_nothing(self):
        bench, url = self.serve({"mind_a": "ok"})
        code, text = self.main(url, "mind_a", "mind_x")
        self.assertEqual(code, 2, text)
        self.assertIn("mind_x: the server knows no such machine", text)
        wrong = self.dir / "wrong_key"
        wrong.write_text("efk_wrong-key-also-never-printed\n")
        code, text = self.main(url, "mind_a", key_file=wrong)
        self.assertEqual(code, 2, text)
        self.assertIn("HTTP 401", text)
        self.assertNotIn("efk_wrong-key-also-never-printed", text)
        self.assertEqual([m for m, _ in bench.requests], ["GET"] * len(bench.requests))

    def test_the_key_goes_over_https_only(self):
        with self.assertRaises(ValueError):
            rollout.Server("http://hesere.uz/effector", KEY)
        rollout.Server("http://127.0.0.1:1/effector", KEY)

    def test_over_the_cable_once_its_log_names_the_machine(self):
        # The server's path is off (denied before a byte reaches the updater): the cable, which names this machine.
        bench, url = self.serve({"mind_a": "denied", "mind_b": "ok"})
        cable = Cable(bench, "mind_a")
        code, text = self.rollout(bench, url, ["mind_a", "mind_b"], cable)
        self.assertEqual(code, 0, text)
        self.assertIn("its agent did not take update_agent (failed: denied): nothing reached its updater: OVER THE CABLE", text)
        self.assertIn("THE CABLE REACHES mind_a", text)
        self.assertEqual(cable.puts[-2:], ["channels/stable", "timestamps/stable"])
        self.assertIn(f"releases/{VERSION}/x86_64/MANIFEST", cable.puts)
        self.assertIn("BACK ON RELEASE", text)
        self.assertEqual(self.updates(bench), ["mind_a", "mind_b"])

    def test_the_cable_reaching_another_machine_sends_nothing(self):
        bench, url = self.serve({"mind_a": "offline", "mind_b": "ok"})
        cable = Cable(bench, "mind_a", named="mind_z")
        code, text = self.rollout(bench, url, ["mind_a", "mind_b"], cable)
        self.assertEqual(code, 1, text)
        self.assertEqual(cable.puts, [])
        self.assertIn("the cable reaches mind_z, not mind_a: nothing sent", text)
        self.assertEqual(self.updates(bench), [])

    def test_a_bundle_is_read_and_checked(self):
        rel = rollout.from_bundle(self.release.bundle, None, self.dir / "unpacked")
        self.assertEqual((rel.version, rel.arch, rel.sha256), (VERSION, "x86_64", self.release.sha256))
        bad = self.dir / "bad.zip"
        with zipfile.ZipFile(bad, "w") as z:
            z.writestr("../escape", b"x")
        with self.assertRaises(rollout.release.ReleaseError):
            rollout.from_bundle(bad, None, self.dir / "bad")


if __name__ == "__main__":
    os.environ.pop("MIND_BOOT_SIGNING_KEY", None)
    os.environ.pop("MIND_RELEASE_KEY", None)
    result = unittest.main(exit=False, verbosity=1).result
    sys.exit(0 if result.wasSuccessful() else 1)
