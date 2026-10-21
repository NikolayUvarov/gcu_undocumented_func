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
    for path in ("AGENTS.md", "issues/README.md", "issues-done/x.done.md", "docs/effector.md", "issues-audit/a.md"):
        assert land.text_only(path), path
    for path in ("scripts/land.py", "issues-audit/repro/x.md", "THIRD_PARTY.md", "kernel/src/main.rs", "docs/figure.png", "LICENSE-MIT"):
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
        def run(self, machine, command, timeout, reason, key):
            self.commands.append(command)
            group = command.split('--group "')[1].split('"')[0]
            return ("completed", 0, "") if group == "build (x86)" else ("completed", 0, f"PASS   5s       {group}\n")
    gate = land.Gate("a" * 40, "t", Effector(), pathlib.Path(os.devnull))
    gate.trees.add("i9")
    gate.lane_run("i9", [("host", "build (x86)"), ("x86", "build (x86 test programs)"), ("x86", "x86: keys, shell, tools"), ("host", "host tests")])
    states = {r["group"]: r["state"] for r in gate.results}
    assert states == {"build (x86)": "ERROR", "build (x86 test programs)": "ERROR", "x86: keys, shell, tools": "ERROR", "host tests": "PASS"}, states
    assert "MIND_SSHD=/bin/true" in gate.effector.commands[0] and "MIND_SSHD" not in gate.effector.commands[-1]


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


if __name__ == "__main__":
    for name, test in list(globals().items()):
        if name.startswith("test_"):
            test()
            print(f"ok {name}")
    print("land_test: all passed")
