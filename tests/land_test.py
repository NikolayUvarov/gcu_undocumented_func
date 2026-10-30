#!/usr/bin/env python3
"""Host test of the landing queue (scripts/land.py, AGENTS.md section 4): requests and answers, the gate's plan over
every group of scripts/ci_local.sh, its verdict, and a candidate built, pushed to `main` and announced on throwaway
repositories (no network, no Effector)."""
import importlib.util, json, os, pathlib, subprocess, sys, tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location("land", ROOT / "scripts/land.py")
land = importlib.util.module_from_spec(spec)
spec.loader.exec_module(land)
ENV = {**os.environ, "GIT_CONFIG_GLOBAL": os.devnull, "GIT_CONFIG_NOSYSTEM": "1", "GIT_AUTHOR_NAME": "test", "GIT_AUTHOR_EMAIL": "t@t",
       "GIT_COMMITTER_NAME": "test", "GIT_COMMITTER_EMAIL": "t@t"}


def test_requests_and_answers():
    files = {
        "2026-10-21T1000Z-LNK-ready-link.md": "# x\n\n**Ready:** `claude/LNK-link` abc1234\n",
        "2026-10-21T1100Z-DRV-ready-hubs.md": "**Ready:** `claude/ASR-DRV` def5678 and more text\n",
        "2026-10-21T1200Z-LNK-ready-link-again.md": "**Ready:** claude/LNK-link 0123abcd\n",
        "2026-10-21T1300Z-KRN-landed-1.md": "**Landed:** `2026-10-21T1100Z-DRV-ready-hubs.md` — `claude/ASR-DRV`\n",
        "2026-10-21T1400Z-APP-a-note.md": "no request here\n",
    }
    requests, superseded = land.pending(files)
    assert requests == [("2026-10-21T1200Z-LNK-ready-link-again.md", "claude/LNK-link", "0123abcd")], requests
    assert superseded == ["2026-10-21T1000Z-LNK-ready-link.md"], superseded


def test_text_only():
    for path in ("AGENTS.md", "issues/README.md", "issues-done/x.done.md", "issues-done/307-STO-0019-x.done", "docs/effector.md", "issues-audit/a.md"):
        assert land.text_only(path), path
    for path in ("scripts/land.py", "issues-audit/repro/x.md", "issues-audit/repro/x.done", "THIRD_PARTY.md", "kernel/src/main.rs", "docs/figure.png", "LICENSE-MIT"):
        assert not land.text_only(path), path


def test_plan_covers_every_group():
    parts = land.ci_groups(ROOT)
    expected = {g for groups in parts.values() for g in groups}
    assert "x86: AVX state" in expected and "aarch64: NVMe boot disk" in expected, expected
    for preset in land.PRESETS:
        layout = land.plan(parts, preset)
        placed = [g for entries in layout.values() for _, g in entries]
        builds = {g for names in land.BUILDS.values() for g in names}
        assert expected <= set(placed), (preset, expected - set(placed))
        assert len([g for g in placed if g not in builds]) == len({g for g in placed if g not in builds}), (preset, "a group twice")
        for lane, entries in layout.items():
            used = {p for p, g in entries if g not in builds}
            names = [g for _, g in entries]
            if "x86" in used:
                assert names.index("build (x86)") < names.index("build (x86 test programs)") < min(names.index(g) for p, g in entries if p == "x86" and g not in builds), (lane, names)
            if "aarch64" in used:
                assert names.index("build (aarch64)") < min(names.index(g) for p, g in entries if p == "aarch64" and g not in builds), (lane, names)
            if "host tests" in names:  # they read the x86 image's ELF files
                assert names.index("build (x86)") < names.index("host tests"), (lane, names)
    part_of = {g: p for p, gs in parts.items() for g in gs}
    assert [g for _, g in land.builds_for({"host"}, part_of)] == ["build (x86)", "build (x86 test programs)"]
    layout = land.plan(parts, "full", lanes=["rz", "pcu"])
    assert set(layout) <= {"rz", "pcu"} and expected <= {g for e in layout.values() for _, g in e}, layout
    runs = [g for _, g in land.runs_for([("x86", "x86: AVX state"), ("x86", "x86: AVX state, one CPU"), ("x86", "x86: no PIT")])]
    assert runs == ["x86: AVX state", "x86: no PIT"], runs


def test_summary_and_verdict():
    out = "PASS  x86: AVX state\nPASS   42s      x86: AVX state\nPASS   33s      x86: AVX state, one CPU\nFAIL   1320s    x86: keys, shell, tools\n"
    assert land.parse_summary(out) == [("PASS", 42, "x86: AVX state"), ("PASS", 33, "x86: AVX state, one CPU"), ("FAIL", 1320, "x86: keys, shell, tools")]
    r = lambda g, s: {"group": g, "state": s}
    assert land.verdict(["a", "b"], [r("a", "PASS"), r("b", "FAIL"), r("b", "PASS")]) == (True, [])
    assert land.verdict(["a", "b"], [r("a", "PASS")])[1] == ["b: not run"]
    assert land.verdict(["a"], [r("a", "FAIL"), r("a", "FAIL"), r("a", "PASS")])[1] == ["a: failed 2 times"]
    assert land.verdict(["a"], [r("a", "PASS"), r("a", "ERROR")])[1] == ["a: ERROR"]



def test_a_failed_build_stops_its_lane():
    class Effector:
        def __init__(self): self.commands = []
        def run(self, machine, command, timeout, reason, key, queue_ttl=None):
            self.commands.append(command)
            group = command.split('--group "')[1].split('"')[0]
            return ("completed", 0, "") if group == "build (x86)" else ("completed", 0, f"PASS   5s       {group}\n")
    gate = land.Gate("a" * 40, "t", Effector(), pathlib.Path(os.devnull))
    gate.trees.add("i9")
    gate.lane_run("i9", [("host", "build (x86)"), ("x86", "build (x86 test programs)"), ("x86", "x86: keys, shell, tools"), ("host", "host tests")])
    states = {r["group"]: r["state"] for r in gate.results}
    assert states == {"build (x86)": "ERROR", "build (x86 test programs)": "ERROR", "x86: keys, shell, tools": "ERROR", "host tests": "PASS"}, states
    assert "MIND_SSHD=/bin/true" in gate.effector.commands[0] and "MIND_SSHD" not in gate.effector.commands[-1]


def test_a_pool_takes_the_longest_first():
    parts = land.ci_groups(ROOT)
    part_of = {g: p for p, gs in parts.items() for g in gs}
    builds = {g for names in land.BUILDS.values() for g in names}
    queue = [(part_of[g], g) for g in part_of if g not in builds]
    for pool, lanes in land.POOLS.items():
        taken, minutes, left = land.simulate(lanes, queue, land.SECONDS)
        assert not left and 0 < minutes < 120, (pool, left, minutes)
        assert sorted(g for gs in taken.values() for g in gs) == sorted(g for _, g in queue), pool  # each group once
        for lane, groups in taken.items():
            assert all(part_of[g] in land.POOL_PARTS[lane.split("-")[0]] for g in groups), (pool, lane, groups)
    queue = [("x86", "a"), ("aarch64", "b"), ("host", "c")]
    assert land.pick(queue, "acc1", {"x86"}, {"a": 100, "b": 400, "c": 900}) == ("aarch64", "b")  # the host part: un-rizen
    assert land.pick(queue, "rz-3", {"x86"}, {"a": 100, "b": 400, "c": 900}) == ("host", "c")
    assert land.pick(queue, "pcu", set(), {"a": 900, "b": 1}) == ("aarch64", "b")  # PCU gates aarch64 alone
    # A group a machine cannot run is left to the others, however long it is.
    assert land.pick([("aarch64", "aarch64: 16 CPUs"), ("aarch64", "b")], "pcu", set(), {"aarch64: 16 CPUs": 900, "b": 1}) == ("aarch64", "b")
    assert land.pick([("aarch64", "aarch64: 16 CPUs")], "rz-2", set(), {}) == ("aarch64", "aarch64: 16 CPUs")
    assert land.pick(queue, "acc2", set(), {"a": 900, "b": 1}, broken={"x86"}) == ("aarch64", "b")
    assert land.pick(queue, "i9", set(), {}, broken={"x86"}) is None
    # A machine Effector lists as off gives no lane (round 47).
    endpoints = [{"id": land.MACHINES["rz"]["agent"], "online": True}, {"id": land.MACHINES["acc1"]["agent"], "online": False, "last_seen": "20:07"}]
    assert land.online_lanes(["rz", "rz-2", "acc1", "pcu"], endpoints) == (["rz", "rz-2"], {"acc1": "20:07", "pcu": "never"})
    # Each machine's load once, before the gate; and how long each lane worked, after it.
    endpoints[0]["load"] = {"cpu_cores": 16, "load_percent": 7.8}
    assert land.machine_loads(["rz", "rz-2", "pcu"], endpoints) == {"rz": "16 cores, 8% busy", "pcu": "? cores, load unknown"}
    results = [{"lane": "rz", "seconds": 600}, {"lane": "rz", "seconds": 300}, {"lane": "pcu", "seconds": 120}, {"lane": "-", "seconds": 0}]
    assert land.lane_minutes(results) == {"rz": (15, 2), "pcu": (2, 1)}


def test_a_pool_gives_a_failed_builds_groups_to_another_lane():
    class Effector:
        def __init__(self, wait=True):
            threading = __import__("threading")
            self.commands, self.lock, self.pcu_failed = [], threading.Lock(), threading.Event()
            if not wait: self.pcu_failed.set()
        def run(self, machine, command, timeout, reason, key, queue_ttl=None):
            group = command.split('--group "')[1].split('"')[0]
            lane = reason.split(" on ")[-1]
            if lane == "acc1":  # acc1 holds one group while PCU takes the other and fails its build, whatever the threads' order
                assert self.pcu_failed.wait(30), "PCU never failed its build"
            with self.lock: self.commands.append((lane, group))
            failed = group == "=build (aarch64)" and lane == "pcu"
            if failed: self.pcu_failed.set()
            return "completed", 0, f"{'FAIL' if failed else 'PASS'}   5s       {group[1:]}\n"
    part_of = {"build (x86)": "host", "build (x86 test programs)": "x86", "build (aarch64)": "aarch64",
               "x86: a": "x86", "x86: b": "x86", "aarch64: c": "aarch64", "aarch64: d": "aarch64"}
    queue = [(p, g) for g, p in part_of.items() if not g.startswith("build")]
    with tempfile.TemporaryDirectory() as temp:
        taken_path = pathlib.Path(temp) / "taken.json"
        gate = land.Gate("a" * 40, "t", Effector(), pathlib.Path(os.devnull), exact=True)
        gate.trees.update({"acc1", "pcu"})
        gate.pool(["acc1", "pcu"], list(queue), part_of, {"aarch64: c": 900, "aarch64: d": 800, "x86: a": 50, "x86: b": 40}, taken_path)
        passed = {r["group"] for r in gate.results if r["state"] == "PASS" and not r["group"].startswith("build")}
        assert passed == {g for _, g in queue}, gate.results
        ran = [(lane, g) for lane, g in gate.effector.commands]
        assert ("pcu", "=build (aarch64)") in ran and not any(lane == "pcu" and not g.startswith("=build") for lane, g in ran), ran
        taken = json.loads(taken_path.read_text())
        assert sorted(taken["acc1"]) == sorted(g for _, g in queue) and not taken.get("pcu"), taken
        # Started again on the same candidate, each lane first takes what it took before, in order: the same keys.
        again = land.Gate("a" * 40, "t", Effector(wait=False), pathlib.Path(os.devnull), exact=True)
        again.trees.update({"acc1", "pcu"})
        again.pool(["acc1", "pcu"], list(queue), part_of, {}, taken_path)
        assert [g for lane, g in again.effector.commands if lane == "acc1" and not g.startswith("=build")] == ["=" + g for g in taken["acc1"]]


def test_a_tree_waits_for_an_earlier_runs_operations_in_its_clone():
    old, other, mine = ({"id": i, "reason": f"{land.TRACK} landing gate {tag}: x86: a on {lane}"}
                        for i, tag, lane in (("1", "land-old", "rz"), ("2", "land-old", "rz-2"), ("3", "land-new", "rz")))

    class Effector:
        def __init__(self):
            self.views, self.runs = [[old, other, mine], [other, mine]], []  # the earlier run's group on rz ends after one look
        def active(self, machine):
            return self.views.pop(0) if len(self.views) > 1 else self.views[0]
        def run(self, machine, command, timeout, reason, key, queue_ttl=None):
            self.runs.append((len(self.views), reason))
            return "completed", 0, "TREE " + "a" * 40
    sleep, land.time.sleep = land.time.sleep, lambda seconds: None
    try:
        gate = land.Gate("a" * 40, "land-new", Effector(), pathlib.Path(os.devnull))
        assert gate.tree("rz") and gate.effector.runs == [(1, f"{land.TRACK} landing gate land-new: the tree on rz")], gate.effector.runs
        assert gate.earlier("rz-2") == ["2"] and gate.earlier("rz") == []
    finally:
        land.time.sleep = sleep


def git(*args, cwd):
    return subprocess.run(["git", *args], cwd=cwd, env=ENV, capture_output=True, text=True, check=True).stdout.strip()


def commit(repo, path, text, message):
    (repo / path).parent.mkdir(parents=True, exist_ok=True)
    (repo / path).write_text(text)
    git("add", path, cwd=repo)
    git("commit", "-q", "-m", message, cwd=repo)
    return git("rev-parse", "HEAD", cwd=repo)


def run_land(repo, work, *args, ok=True):
    env = {**ENV, "LAND_REPO": str(repo), "LAND_WORK": str(work), "LAND_TRACK": "KRN", "LAND_TRAILER": "Trailer: test"}
    r = subprocess.run([sys.executable, str(ROOT / "scripts/land.py"), *args], env=env, capture_output=True, text=True)
    assert (r.returncode == 0) == ok, (args, r.returncode, r.stdout, r.stderr)
    return r.stdout + r.stderr


def test_candidate_push_and_announce():
    with tempfile.TemporaryDirectory() as temp:
        temp = pathlib.Path(temp)
        origin, repo, work = temp / "origin.git", temp / "repo", temp / "work"
        git("init", "-q", "--bare", str(origin), cwd=temp)
        git("init", "-q", "-b", "main", str(repo), cwd=temp)
        git("remote", "add", "origin", str(origin), cwd=repo)
        commit(repo, "x.txt", "base\n", "base")
        git("push", "-q", "origin", "main", cwd=repo)
        shas = {}
        for branch, path, text in (("claude/A", "a.txt", "a\n"), ("claude/B", "b.txt", "b\n"), ("claude/C", "x.txt", "c\n")):
            git("checkout", "-q", "-b", branch, "main", cwd=repo)
            shas[branch] = commit(repo, path, text, branch)
            git("push", "-q", "origin", branch, cwd=repo)
        git("checkout", "-q", "main", cwd=repo)
        commit(repo, "x.txt", "main moved\n", "main moves")
        git("push", "-q", "origin", "main", cwd=repo)
        git("checkout", "-q", "--orphan", "internal-dev-messages", cwd=repo)
        git("rm", "-q", "-rf", ".", cwd=repo)
        for n, branch in enumerate(("claude/A", "claude/B", "claude/C")):
            commit(repo, f"LANDING/2026-10-21T10{n}0Z-T{n}-ready-x.md", f"**Ready:** `{branch}` {shas[branch]}\n", f"request {n}")
        git("push", "-q", "origin", "internal-dev-messages", cwd=repo)
        git("checkout", "-q", "main", cwd=repo)

        assert "claude/C" in run_land(repo, work, "queue")
        run_land(repo, work, "candidate")
        state = json.loads((work / "candidate.json").read_text())
        assert [e["branch"] for e in state["landed"]] == ["claude/A", "claude/B"], state
        assert [e["branch"] for e in state["returned"]] == ["claude/C"] and "conflict" in state["returned"][0]["why"], state
        tree = work / "candidate"
        assert (tree / "a.txt").exists() and (tree / "b.txt").exists() and (tree / "x.txt").read_text() == "main moved\n"
        assert "Trailer: test" in git("log", "-1", "--format=%B", cwd=tree)

        assert "no green gate" in run_land(repo, work, "push", ok=False)
        candidate = state["candidate"]
        (work / f"gate-{candidate[:12]}.json").write_text(json.dumps({"commit": candidate, "green": True}))
        # Text only on main meanwhile: merged in without a gate.
        git("fetch", "-q", "origin", "main", cwd=repo)
        git("checkout", "-q", "--detach", "origin/main", cwd=repo)
        commit(repo, "issues/note.md", "closed\n", "text only")
        git("push", "-q", "origin", "HEAD:main", cwd=repo)
        run_land(repo, work, "push")
        main = git("rev-parse", "main", cwd=origin)
        assert git("merge-base", "--is-ancestor", candidate, main, cwd=origin) == "" and git("show", f"{main}:issues/note.md", cwd=origin) == "closed"

        # Code on main after the candidate: refused.
        run_land(repo, work, "candidate")
        state = json.loads((work / "candidate.json").read_text())
        (work / f"gate-{state['candidate'][:12]}.json").write_text(json.dumps({"commit": state["candidate"], "green": True}))
        git("fetch", "-q", "origin", "main", cwd=repo)
        git("checkout", "-q", "--detach", "origin/main", cwd=repo)
        commit(repo, "src.rs", "fn main() {}\n", "code")
        git("push", "-q", "origin", "HEAD:main", cwd=repo)
        assert "main moved with code" in run_land(repo, work, "push", ok=False)

        run_land(repo, work, "announce", "--return", "2026-10-21T1020Z-T2-ready-x.md=conflicts with main")

        # One request that holds main already is the candidate as it is, published on its own branch.
        git("checkout", "-q", "-b", "claude/D", "origin/main", cwd=repo)
        d = commit(repo, "d.txt", "d\n", "claude/D")
        git("push", "-q", "origin", "claude/D", cwd=repo)
        git("fetch", "-q", "origin", "internal-dev-messages", cwd=repo)
        git("checkout", "-q", "--detach", "origin/internal-dev-messages", cwd=repo)
        commit(repo, "LANDING/2026-10-21T1100Z-T3-ready-d.md", f"**Ready:** `claude/D` {d}\n", "request d")
        git("push", "-q", "origin", "HEAD:internal-dev-messages", cwd=repo)
        out = run_land(repo, work, "candidate", "--exclude", "claude/C")
        state = json.loads((work / "candidate.json").read_text())
        assert state["candidate"] == d and state["published"] == "claude/D", (state, out)
        files = git("ls-tree", "--name-only", "internal-dev-messages", "LANDING/", cwd=origin).splitlines()
        answer = next(f for f in files if "-KRN-" in f)
        text = git("show", f"internal-dev-messages:{answer}", cwd=origin)
        assert "**Returned:** `2026-10-21T1020Z-T2-ready-x.md`" in text, text


def test_a_text_conflict_waits_for_a_hand_merge():
    with tempfile.TemporaryDirectory() as temp:
        temp = pathlib.Path(temp)
        origin, repo, work = temp / "origin.git", temp / "repo", temp / "work"
        git("init", "-q", "--bare", str(origin), cwd=temp)
        git("init", "-q", "-b", "main", str(repo), cwd=temp)
        git("remote", "add", "origin", str(origin), cwd=repo)
        commit(repo, "issues/README.md", "| base |\n", "base")
        git("push", "-q", "origin", "main", cwd=repo)
        git("checkout", "-q", "-b", "claude/A", cwd=repo)
        commit(repo, "src.rs", "fn a() {}\n", "code")
        a = commit(repo, "issues/README.md", "| base |\n| A |\n", "claude/A")
        git("push", "-q", "origin", "claude/A", cwd=repo)
        git("checkout", "-q", "--orphan", "internal-dev-messages", cwd=repo)
        git("rm", "-q", "-rf", ".", cwd=repo)
        commit(repo, "LANDING/2026-10-21T1000Z-T0-ready-a.md", f"**Ready:** `claude/A` {a}\n", "request")
        git("push", "-q", "origin", "internal-dev-messages", cwd=repo)
        git("checkout", "-q", "main", cwd=repo)
        run_land(repo, work, "candidate")
        (work / f"gate-{a[:12]}.json").write_text(json.dumps({"commit": a, "green": True}))
        commit(repo, "issues/README.md", "| base |\n| main |\n", "text only, the same lines")
        git("push", "-q", "origin", "main", cwd=repo)

        tree = work / "candidate"
        assert "resolve the merge in" in run_land(repo, work, "push", ok=False)
        assert git("rev-parse", "main", cwd=origin) != a
        # A hand merge that brings code is refused.
        (tree / "issues/README.md").write_text("| base |\n| A |\n| main |\n")
        (tree / "src.rs").write_text("fn b() {}\n")
        git("commit", "-q", "-am", "merge", cwd=tree)
        assert "changes code (src.rs)" in run_land(repo, work, "push", ok=False)
        git("reset", "-q", "--hard", a, cwd=tree)
        subprocess.run(["git", "merge", "-q", "origin/main"], cwd=tree, env=ENV, capture_output=True)
        (tree / "issues/README.md").write_text("| base |\n| A |\n| main |\n")
        git("commit", "-q", "-am", "merge", cwd=tree)
        run_land(repo, work, "push")
        assert git("show", "main:issues/README.md", cwd=origin) == "| base |\n| A |\n| main |"
        assert json.loads((work / "candidate.json").read_text())["landed_as"] == git("rev-parse", "main", cwd=origin)


def test_a_text_conflict_is_resolved_once_and_code_returns():
    with tempfile.TemporaryDirectory() as temp:
        temp = pathlib.Path(temp)
        origin, repo, work = temp / "origin.git", temp / "repo", temp / "work"
        git("init", "-q", "--bare", str(origin), cwd=temp)
        git("init", "-q", "-b", "main", str(repo), cwd=temp)
        git("remote", "add", "origin", str(origin), cwd=repo)
        commit(repo, "issues/README.md", "| base |\n\nRequests: none.\n", "base")
        commit(repo, "src.rs", "fn base() {}\n", "base code")
        git("push", "-q", "origin", "main", cwd=repo)
        shas = {}
        for branch, path, text in (("claude/A", "issues/README.md", "| base |\n| A |\n\nRequests: from A.\n"), ("claude/B", "src.rs", "fn b() {}\n")):
            git("checkout", "-q", "-b", branch, "main", cwd=repo)
            commit(repo, f"{branch[-1]}.rs", "fn x() {}\n", "code")
            shas[branch] = commit(repo, path, text, branch)
            git("push", "-q", "origin", branch, cwd=repo)
        git("checkout", "-q", "main", cwd=repo)
        commit(repo, "issues/README.md", "| base |\n\nRequests: from main.\n", "text")
        commit(repo, "src.rs", "fn main_moved() {}\n", "code")
        git("push", "-q", "origin", "main", cwd=repo)
        git("checkout", "-q", "--orphan", "internal-dev-messages", cwd=repo)
        git("rm", "-q", "-rf", ".", cwd=repo)
        for n, branch in enumerate(shas):
            commit(repo, f"LANDING/2026-10-21T10{n}0Z-T{n}-ready-x.md", f"**Ready:** `{branch}` {shas[branch]}\n", f"request {n}")
        git("push", "-q", "origin", "internal-dev-messages", cwd=repo)
        git("checkout", "-q", "main", cwd=repo)

        run_land(repo, work, "candidate")
        state = json.loads((work / "candidate.json").read_text())
        assert not state["landed"] and [e["branch"] for e in state["skipped"]] == ["claude/A"], state
        assert [e["branch"] for e in state["returned"]] == ["claude/B"] and "(src.rs)" in state["returned"][0]["why"], state
        request = state["skipped"][0]["request"]
        assert "issues/README.md" in run_land(repo, work, "resolve", request)
        resolve = work / "resolve"
        assert "<<<<<<<" in (resolve / "issues/README.md").read_text()
        assert "markers left" in run_land(repo, work, "resolve", "--done", request, ok=False)
        (resolve / "issues/README.md").write_text("| base |\n| A |\n\nRequests: from main; from A.\n")
        run_land(repo, work, "resolve", "--done", request)

        run_land(repo, work, "candidate", "--exclude", "claude/B")
        state = json.loads((work / "candidate.json").read_text())
        assert [e["branch"] for e in state["landed"]] == ["claude/A"], state
        tree = work / "candidate"
        assert (tree / "issues/README.md").read_text() == "| base |\n| A |\n\nRequests: from main; from A.\n"
        assert (tree / "A.rs").exists() and "resolved as the integrator recorded it" in git("log", "-1", "--format=%B", cwd=tree)


if __name__ == "__main__":
    for name, test in list(globals().items()):
        if name.startswith("test_"):
            test()
            print(f"ok {name}")
    print("land_test: all passed")
