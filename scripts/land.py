#!/usr/bin/env python3
"""The landing queue (AGENTS.md, section 4): code reaches `main` as a candidate the integrator builds and gates once.

  scripts/land.sh queue                       the landing requests not answered yet, oldest first
  scripts/land.sh candidate [--exclude BRANCH]... [--push]
                                              origin/main merged with each pending request in order; a conflict is left
                                              out; --push publishes it on the integrator's branch for the machines
  scripts/land.sh gate [--preset P] [--lanes L,...] [--rerun-lane L] [--dry-run]
                                              every group of scripts/ci_local.sh on the candidate, spread over the
                                              Effector machines; a group that failed runs once more at the end
  scripts/land.sh push                        main := the candidate, after a green gate of that very commit; text that
                                              main gained meanwhile is merged in, by hand where it conflicts
  scripts/land.sh announce [--return REQUEST=REASON]...
                                              one answer in LANDING/ naming each request landed, returned or superseded

Requests are messages on `internal-dev-messages`: LANDING/<YYYY-MM-DDTHHMMZ>-<TRK>-ready-<name>.md with the line
**Ready:** `<branch>` <commit>. Answers there name them in **Landed:**, **Returned:** or **Superseded:** lines.
State: $LAND_WORK (default $TMPDIR/mind-land). Effector's API key: the file $EFFECTOR_KEY_FILE, sent only in the
X-API-Key header and never printed (docs/effector.md, section 2). $LAND_TRAILER is appended to the merge messages.
"""
import argparse, datetime, hashlib, json, os, pathlib, re, subprocess, sys, threading, time, urllib.error, urllib.request

ROOT = pathlib.Path(os.environ.get("LAND_REPO") or pathlib.Path(__file__).resolve().parent.parent)
WORK = pathlib.Path(os.environ.get("LAND_WORK") or pathlib.Path(os.environ.get("TMPDIR", "/tmp")) / "mind-land")
MESSAGES = "internal-dev-messages"
CANDIDATE_BRANCH = os.environ.get("LAND_BRANCH", "claude/KRN-landing")
TRACK = os.environ.get("LAND_TRACK", "KRN")
URL = os.environ.get("LAND_URL", "https://github.com/NikolayUvarov/gcu_undocumented_func")
TRAILER = os.environ.get("LAND_TRAILER", "")
EFFECTOR = os.environ.get("EFFECTOR_URL", "https://hesere.uz/effector")

REQUEST = re.compile(r"^\*\*Ready:\*\*\s+`?([^`\s]+)`?\s+([0-9a-f]{7,40})\b", re.M)
ANSWER = re.compile(r"^\*\*(Landed|Returned|Superseded):\*\*\s+`?([^`\s]+\.md)`?", re.M)
SUMMARY = re.compile(r"^(PASS|FAIL)\s+(\d+)s\s+(.+?)\s*$", re.M)


def git(*args, cwd=None, check=True):
    run = subprocess.run(["git", *args], cwd=cwd or ROOT, capture_output=True, text=True)
    if check and run.returncode != 0:
        raise SystemExit(f"git {' '.join(args)}: {run.stderr.strip()}")
    return run.stdout.strip() if check else run


# --- The queue ---------------------------------------------------------------------------------------------------

def pending(files):
    """Requests not answered, oldest first, and the older requests of a branch that a newer one superseded."""
    answered = {name for text in files.values() for _, name in ANSWER.findall(text)}
    latest, superseded = {}, []
    for name in sorted(files):
        match = REQUEST.search(files[name])
        if not match or name in answered:
            continue
        branch, commit = match.groups()
        if branch in latest:
            superseded.append(latest[branch][0])
        latest[branch] = (name, branch, commit)
    return sorted(latest.values()), sorted(superseded)


def queue_files():
    git("fetch", "-q", "origin", MESSAGES)
    names = git("ls-tree", "--name-only", f"origin/{MESSAGES}", "LANDING/").splitlines()
    return {pathlib.PurePath(n).name: git("show", f"origin/{MESSAGES}:{n}") for n in names if n.endswith(".md")}


def cmd_queue(_args):
    requests, superseded = pending(queue_files())
    for name, branch, commit in requests:
        print(f"{name}  {branch}  {commit[:12]}")
    for name in superseded:
        print(f"{name}  (superseded by a newer request of its branch)")
    if not requests:
        print("no pending landing request")


# --- The candidate -----------------------------------------------------------------------------------------------

def state_path():
    return WORK / "candidate.json"


def load_state():
    path = state_path()
    if not path.exists():
        raise SystemExit("no candidate: run `scripts/land.sh candidate` first")
    return json.loads(path.read_text())


def worktree(name, ref):
    path = WORK / name
    if not (path / ".git").exists():
        WORK.mkdir(parents=True, exist_ok=True)
        git("worktree", "add", "--detach", "-f", str(path), ref)
    else:
        git("merge", "--abort", cwd=path, check=False)
        git("reset", "-q", "--hard", cwd=path)
        git("checkout", "-q", "--detach", ref, cwd=path)
    return path


def cmd_candidate(args):
    requests, superseded = pending(queue_files())
    git("fetch", "-q", "origin", "main")
    base = git("rev-parse", "origin/main")
    tree = worktree("candidate", base)
    landed, returned, skipped = [], [], []
    for name, branch, commit in requests:
        if branch in args.exclude:
            skipped.append({"request": name, "branch": branch, "why": "left out by the integrator"}); continue
        if git("fetch", "-q", "origin", branch, check=False).returncode != 0:
            returned.append({"request": name, "branch": branch, "why": f"no branch {branch} on origin"}); continue
        full = git("rev-parse", "-q", "--verify", f"{commit}^{{commit}}", check=False)
        if full.returncode != 0 or git("merge-base", "--is-ancestor", commit, f"origin/{branch}", check=False).returncode != 0:
            returned.append({"request": name, "branch": branch, "why": f"{commit} is not on origin/{branch}"}); continue
        commit = full.stdout.strip()
        if git("merge-base", "--is-ancestor", commit, "HEAD", cwd=tree, check=False).returncode == 0:
            skipped.append({"request": name, "branch": branch, "commit": commit, "why": "already in main or the candidate"}); continue
        message = f"Land {branch} ({commit[:12]}) through the landing queue\n\nRequest: LANDING/{name}" + (f"\n\n{TRAILER}" if TRAILER else "")
        # The first request that already holds main is taken as it is: no merge commit, and it is on origin already.
        fast = not landed and git("merge-base", "--is-ancestor", "HEAD", commit, cwd=tree, check=False).returncode == 0
        merged = git("merge", "--ff-only", "-q", commit, cwd=tree, check=False) if fast else git("merge", "--no-ff", "-q", "-m", message, commit, cwd=tree, check=False)
        if merged.returncode != 0:
            git("merge", "--abort", cwd=tree, check=False)
            returned.append({"request": name, "branch": branch, "commit": commit, "why": "merge conflict with main and the requests before it"}); continue
        landed.append({"request": name, "branch": branch, "commit": commit})
    head = git("rev-parse", "HEAD", cwd=tree)
    # Published already when the candidate is one request's own commit, taken as it is.
    published = next((e["branch"] for e in landed if e["commit"] == head), None)
    state = {"base": base, "candidate": head, "published": published, "landed": landed, "returned": returned,
             "skipped": skipped, "superseded": superseded, "built": datetime.datetime.now(datetime.timezone.utc).isoformat()}
    state_path().write_text(json.dumps(state, indent=1))
    print(f"candidate {state['candidate'][:12]} on main {base[:12]}")
    for entry in landed: print(f"  merged    {entry['branch']} {entry['commit'][:12]} ({entry['request']})")
    for entry in returned: print(f"  returned  {entry['branch']}: {entry['why']} ({entry['request']})")
    for entry in skipped: print(f"  skipped   {entry['branch']}: {entry['why']} ({entry['request']})")
    if published:
        print(f"the candidate is {published} as it is, on origin already")
    elif args.push and landed:
        # The integrator's own branch, rebuilt from main each round: only it uses this branch, so it is replaced.
        git("push", "-q", "-f", "origin", f"{state['candidate']}:refs/heads/{CANDIDATE_BRANCH}")
        state["published"] = CANDIDATE_BRANCH
        state_path().write_text(json.dumps(state, indent=1))
        print(f"pushed to {CANDIDATE_BRANCH}")


# --- The gate over Effector's machines ---------------------------------------------------------------------------

# Machines of docs/effector.md, section 1: who runs the work there, and what an operation sets up first.
MACHINES = {
    "i9": {"agent": "i9_4e6882212251", "run_as": None,
           "env": "export NO_PROXY=127.0.0.1,localhost,::1 no_proxy=127.0.0.1,localhost,::1"},
    "rz": {"agent": "un-rizen_2a8b108d5593", "run_as": "un", "env": ""},
    "pcu": {"agent": "PCU_585240b00e8a", "run_as": "un", "env": ""},
}
# Lanes: one queue of the track on one machine ("rz-2": its second clone there, docs/effector.md, section 3, rule 1).
# Each preset is a measured split; a group no preset names runs on its part's default lane.
PRESETS = {
    # Gate g0141: i9 the long x86 groups, un-rizen the host part, the short x86 groups and most of aarch64.
    "full": {
        "i9": ["x86: keys, shell, tools", "x86: boot, display, network, TLS, shell, memory, clock", "x86: files and block writes",
               "x86: services, storage, audio", "x86: scheduling, isolation, heap", "x86: NVMe boot disk", "x86: 16 CPUs",
               "x86: RAM above 4 GiB", "x86: reproducible build"],
        "rz": ["network driver without legacy", "host tests", "models (TLC)", "x86: one CPU", "x86: AVX state", "x86: AVX state, one CPU", "x86: no PIT",
               "x86: x2APIC", "x86: padded vector area", "x86: protection probes", "x86: TSC measured again", "x86: the watchdog",
               "x86: USB image", "x86: Secure Boot with our keys"],
        "rz-2": ["aarch64: boot and fault containment", "aarch64: programs, shell and four CPUs", "aarch64: files, network and TLS",
                 "aarch64: 16 CPUs"],
        "pcu": ["aarch64: RAM, ACPI and PCI above 4 GiB", "aarch64: GICv2 with GICv2m", "aarch64: NVMe boot disk"],
        "default": {"host": "rz", "x86": "i9", "aarch64": "rz-2"},
    },
    # Gate g0142: while another track's gate loads i9, its groups go to un-rizen's two lanes.
    "no-i9": {
        "rz": ["network driver without legacy", "host tests", "models (TLC)", "x86: keys, shell, tools",
               "x86: boot, display, network, TLS, shell, memory, clock", "x86: one CPU", "x86: AVX state", "x86: AVX state, one CPU", "x86: no PIT", "x86: x2APIC",
               "x86: padded vector area", "x86: protection probes", "x86: TSC measured again", "x86: the watchdog", "x86: USB image",
               "x86: Secure Boot with our keys"],
        "rz-2": ["aarch64: boot and fault containment", "aarch64: programs, shell and four CPUs", "aarch64: files, network and TLS",
                 "aarch64: 16 CPUs", "x86: files and block writes", "x86: services, storage, audio", "x86: scheduling, isolation, heap",
                 "x86: NVMe boot disk", "x86: 16 CPUs", "x86: RAM above 4 GiB", "x86: reproducible build"],
        "pcu": ["aarch64: RAM, ACPI and PCI above 4 GiB", "aarch64: GICv2 with GICv2m", "aarch64: NVMe boot disk"],
        "default": {"host": "rz", "x86": "rz", "aarch64": "rz-2"},
    },
}
BUILDS = {"x86": ["build (x86)", "build (x86 test programs)"], "aarch64": ["build (aarch64)"]}


def ci_groups(tree):
    """{part: [group]} as `scripts/ci_local.sh --list` names them on this tree."""
    parts = {}
    for part in ("host", "x86", "aarch64"):
        out = subprocess.run(["bash", "scripts/ci_local.sh", "--list", "--only", part], cwd=tree, capture_output=True, text=True, check=True)
        parts[part] = [line for line in out.stdout.splitlines() if line.strip()]
    return parts


def plan(parts, preset, lanes=None):
    """{lane: [(part, group)]}: each lane's builds first, then its groups; every group of `parts` placed once."""
    chosen = PRESETS[preset]
    part_of = {group: part for part, groups in parts.items() for group in groups}
    builds = {g for names in BUILDS.values() for g in names}
    placed, out = set(), {}
    for lane, groups in chosen.items():
        if lane == "default" or (lanes and lane not in lanes):
            continue
        out[lane] = [(part_of[g], g) for g in groups if g in part_of]
        placed.update(g for _, g in out[lane])
    for group, part in part_of.items():
        if group in placed or group in builds:
            continue
        lane = chosen["default"][part]
        if lanes and lane not in lanes:
            lane = next(iter(out))
        out.setdefault(lane, []).append((part, group))
    for lane, entries in out.items():
        out[lane] = builds_for(lane_parts(entries), part_of) + entries
    return out


def lane_parts(entries):
    return {part for part, _ in entries}


def builds_for(parts, part_of, done=()):
    """The build groups a lane runs before groups of these parts: the x86 image and test programs, the aarch64 image."""
    return [(part_of[b], b) for part in ("x86", "aarch64") if part in parts for b in BUILDS[part] if b in part_of and b not in done]


def runs_for(entries):
    """The groups a lane passes to `--group`: one whose name holds an earlier one's already ran with it."""
    done, runs = [], []
    for part, group in entries:
        if any(earlier in group for earlier in done):
            continue
        done.append(group); runs.append((part, group))
    return runs


def parse_summary(text):
    """[(PASS|FAIL, seconds, group)] from the table `scripts/ci_local.sh` prints at its end."""
    return [(state, int(secs), name) for state, secs, name in SUMMARY.findall(text)]


def verdict(expected, results):
    """Green when every expected group passed last and failed at most once (one run again, docs/effector.md)."""
    problems = []
    for group in expected:
        states = [r["state"] for r in results if r["group"] == group]
        if not states:
            problems.append(f"{group}: not run")
        elif states[-1] != "PASS":
            problems.append(f"{group}: {states[-1]}")
        elif states.count("FAIL") > 1:
            problems.append(f"{group}: failed {states.count('FAIL')} times")
    return not problems, problems


class Effector:
    FINAL = {"completed", "failed", "timeout", "expired", "cancelled", "rollback", "run_as_unavailable",
             "interactive_session_unavailable", "denied"}

    def __init__(self):
        key_file = os.environ.get("EFFECTOR_KEY_FILE")
        if not key_file or not pathlib.Path(key_file).is_file():
            raise SystemExit("EFFECTOR_KEY_FILE must name the file that holds your Effector API key (docs/effector.md, section 2)")
        self.key_file = pathlib.Path(key_file)

    def call(self, method, path, body=None):
        data = None if body is None else json.dumps(body).encode()
        request = urllib.request.Request(EFFECTOR + path, data=data, method=method)
        request.add_header("X-API-Key", self.key_file.read_text().strip())
        request.add_header("Content-Type", "application/json")
        for attempt in range(8):
            try:
                with urllib.request.urlopen(request, timeout=60) as reply:
                    return json.load(reply)
            except urllib.error.HTTPError as error:
                if error.code < 500 or attempt == 7:
                    raise SystemExit(f"Effector {method} {path.split('?')[0]}: HTTP {error.code}")
            except (urllib.error.URLError, ConnectionError, TimeoutError, OSError):
                if attempt == 7:
                    raise
            time.sleep(min(2 ** attempt, 60))  # a POST carries its idempotency key, so a repeat is safe

    def run(self, machine, command, timeout, reason, key):
        body = {"agent_id": machine["agent"], "kind": "exec", "command": command, "timeout_seconds": timeout, "reason": reason,
                "idempotency_key": key, "queue_ttl_seconds": 6 * 3600, **({"run_as": machine["run_as"]} if machine["run_as"] else {})}
        op = self.call("POST", "/api/v1/operations", body)
        oid = op.get("id") or op.get("operation_id")
        while op.get("status") not in self.FINAL:
            time.sleep(5)
            try:
                op = self.call("GET", f"/api/v1/operations?id={oid}")
            except Exception:  # the connection drops now and then; the operation runs on
                continue
        result = op.get("result") or {}
        return op.get("status"), result.get("exit_code"), (result.get("stdout") or "") + (result.get("stderr") or "")


class Gate:
    def __init__(self, commit, tag, effector, out):
        self.commit, self.tag, self.effector, self.out = commit, tag, effector, out
        self.results, self.lock, self.trees = [], threading.Lock(), set()

    def lane(self, name):
        machine, _, n = name.partition("-")
        suffix = f"-{n}" if n else ""
        home = f"/home/un/mind-core/{TRACK}{suffix}"
        return MACHINES[machine], f"{home}/gcu_undocumented_func", f"/tmp/mind-{TRACK}{suffix}"

    def prelude(self, name):
        machine, clone, tmp = self.lane(name)
        return (f"{machine['env']}\nexport TMPDIR={tmp} GIT_TERMINAL_PROMPT=0; mkdir -p \"$TMPDIR\"\n"
                f". \"$HOME/.cargo/env\" 2>/dev/null\n")

    def submit(self, machine, command, timeout, reason, key):
        try:
            return self.effector.run(machine, command, timeout, reason, key)
        except (Exception, SystemExit) as error:  # counted as a group not run, not as a pass
            return "error", None, str(error)

    def log(self, text):
        with self.lock:
            print(f"{time.strftime('%H:%M:%S')} {text}", flush=True)

    def tree(self, name):
        machine, clone, _ = self.lane(name)
        command = self.prelude(name) + (
            f"[ -d {clone}/.git ] || {{ mkdir -p {os.path.dirname(clone)} && git clone -q {URL} {clone}; }} || exit 3\n"
            f"cd {clone} || exit 3\ngit merge --abort 2>/dev/null; git reset -q --hard\n"
            f"git fetch -q origin && git checkout -q --detach {self.commit} || exit 4\necho \"TREE $(git rev-parse HEAD)\"\n")
        status, code, output = self.submit(machine, command, 900, f"{TRACK} landing gate {self.tag}: the tree on {name}",
                                           f"land-{self.tag}-{name}-tree")
        ok = status == "completed" and code == 0 and f"TREE {self.commit}" in output
        self.log(f"{name}: tree {'ready' if ok else f'FAILED ({status}, exit {code})'}")
        if ok:
            self.trees.add(name)
        return ok

    def group(self, name, part, group, n):
        machine, clone, _ = self.lane(name)
        logs = f"/home/un/mind-core/{TRACK}/logs/{self.tag}/{name}-{n:02d}"
        command = self.prelude(name) + (
            f"cd {clone} || exit 3\n[ \"$(git rev-parse HEAD)\" = \"{self.commit}\" ] || {{ echo \"TREE MOVED\"; exit 5; }}\n"
            # Only the SSH client's host tests need sshd; a machine without it (i9) still runs the host part's other groups.
            + (f"[ -x \"${{MIND_SSHD:-/usr/sbin/sshd}}\" ] || export MIND_SSHD=/bin/true\n" if part == "host" and group != "host tests" else "")
            + f"scripts/ci_local.sh --only {part} --group \"{group}\" > \"$TMPDIR/land-group.out\" 2>&1\n"
            f"grep -E '^(PASS|FAIL) +[0-9]+s ' \"$TMPDIR/land-group.out\"\n"
            f"mkdir -p {logs}; cp \"$TMPDIR/land-group.out\" \"$TMPDIR\"/mind-ci-local/*.log {logs}/ 2>/dev/null\n"
            f"if grep -q '^FAIL ' \"$TMPDIR/land-group.out\"; then f=$(ls \"$TMPDIR\"/mind-ci-local/01-*.log 2>/dev/null | head -1);"
            f" echo \"--- {logs}\"; tail -n 20 \"$f\" 2>/dev/null | cut -c1-300; fi\ntrue\n")
        key = f"land-{self.tag}-{name}-{n:02d}-" + hashlib.sha256(command.encode()).hexdigest()[:12]  # a changed command is a new operation
        status, code, output = self.submit(machine, command, 3500, f"{TRACK} landing gate {self.tag}: {group} on {name}", key)
        rows = parse_summary(output)
        if not rows:
            rows = [("ERROR", 0, group)]
            self.log(f"{name}: {group}: no result ({status}, exit {code}): {output[-500:]}")
        with self.lock:
            for state, secs, ran in rows:
                self.results.append({"group": ran, "state": state, "seconds": secs, "lane": name})
        for state, secs, ran in rows:
            self.log(f"{name}: {state} {secs}s {ran}" + ("" if state == "PASS" else f"\n{output[-1500:]}"))
        return rows

    def lane_run(self, name, entries, start=1):
        if name not in self.trees and not self.tree(name):
            with self.lock:
                self.results.extend({"group": g, "state": "ERROR", "seconds": 0, "lane": name} for _, g in entries)
            return
        broken = set()  # parts whose build failed here: their groups would test the clone's previous image
        builds = {b: part for part, names in BUILDS.items() for b in names}
        for n, (part, group) in enumerate(runs_for(entries), start):
            if part in broken or (group in builds and builds[group] in broken):
                with self.lock:
                    self.results.append({"group": group, "state": "ERROR", "seconds": 0, "lane": name})
                self.log(f"{name}: not run, its build failed here: {group}")
                continue
            rows = self.group(name, part, group, n)
            if group in builds and any(state != "PASS" for state, _, _ in rows):
                broken.add(builds[group])

    def save(self, expected, problems):
        self.out.write_text(json.dumps({"commit": self.commit, "tag": self.tag, "green": not problems, "problems": problems,
                                        "expected": expected, "results": self.results}, indent=1))


def cmd_gate(args):
    state = load_state()
    commit = state["candidate"]
    tree = WORK / "candidate"
    if git("rev-parse", "HEAD", cwd=tree) != commit:
        raise SystemExit(f"{tree} is not at the candidate {commit[:12]}")
    parts = ci_groups(tree)
    part_of = {group: part for part, groups in parts.items() for group in groups}
    lanes = args.lanes.split(",") if args.lanes else None
    layout = plan(parts, args.preset, lanes)
    expected = [g for groups in parts.values() for g in groups]
    for lane, entries in layout.items():
        print(f"{lane}: " + "; ".join(g for _, g in runs_for(entries)))
    if args.dry_run:
        return
    published = state.get("published")
    if not published or git("ls-remote", "origin", published).split()[:1] != [commit]:
        raise SystemExit(f"the candidate {commit[:12]} is not on origin: run `scripts/land.sh candidate --push`")
    tag = args.tag or "land-" + commit[:8]
    out = WORK / f"gate-{commit[:12]}.json"
    gate = Gate(commit, tag, Effector(), out)
    threads = [threading.Thread(target=gate.lane_run, args=(lane, entries)) for lane, entries in layout.items()]
    for thread in threads: thread.start()
    for thread in threads: thread.join()
    _, problems = verdict(expected, gate.results)
    # A group that failed runs once more, alone, before the gate is called red (docs/effector.md, section 1).
    again = [g for g in expected if any(p.startswith(g + ":") for p in problems)]
    for i, group in enumerate(again):
        part = part_of[group]
        lane = args.rerun_lane or next((r["lane"] for r in gate.results if r["group"] == group), next(iter(layout)))
        built = {r["group"] for r in gate.results if r["lane"] == lane and r["state"] == "PASS"}
        gate.lane_run(lane, builds_for({part}, part_of, built) + [(part, group)], start=90 + 5 * i)
    green, problems = verdict(expected, gate.results)
    gate.save(expected, problems)
    print(f"gate {tag} on {commit[:12]}: {'GREEN' if green else 'RED'}; record {out}")
    for problem in problems:
        print(f"  {problem}")
    sys.exit(0 if green else 1)


# --- Main and the answers ----------------------------------------------------------------------------------------

TEXT_DIRS = ("issues/", "issues-done/", "issues-human/", "issues-critica/", "issues-critica-done/", "docs/", "constitution/")
TEXT_FILES = {"AGENTS.md", "CONTRIBUTING.md", "TRACKS.md", "README.md", "ROADMAP.md", "ROADMAP_RU.md", "SECURITY.md", "CODE_OF_CONDUCT.md"}


def text_only(path):
    """Whether a changed file is one of AGENTS.md's text-only files, which need no gate."""
    if path in TEXT_FILES:
        return True
    if path.startswith("issues-audit/") and "/repro/" not in path:
        return path.endswith(".md")
    return path.startswith(TEXT_DIRS) and path.endswith(".md")


def cmd_push(_args):
    state = load_state()
    commit, base = state["candidate"], state["base"]
    record = WORK / f"gate-{commit[:12]}.json"
    if not record.exists() or not json.loads(record.read_text()).get("green"):
        raise SystemExit(f"no green gate recorded for {commit[:12]} ({record})")
    git("fetch", "-q", "origin", "main")
    main = git("rev-parse", "origin/main")
    tree = WORK / "candidate"
    if main != base:
        if git("merge-base", "--is-ancestor", base, main, check=False).returncode != 0:
            raise SystemExit("main was rewritten since the candidate was built")
        changed = git("diff", "--name-only", base, main).splitlines()
        code = [path for path in changed if not text_only(path)]
        if code:
            raise SystemExit(f"main moved with code since the candidate ({', '.join(code[:5])}): build and gate a new one")
        if git("merge-base", "--is-ancestor", main, "HEAD", cwd=tree, check=False).returncode != 0:
            # Text only: merged in without a gate (AGENTS.md, section 4).
            message = f"Merge main ({main[:12]}, text only) into the landing candidate" + (f"\n\n{TRAILER}" if TRAILER else "")
            if git("merge", "--no-ff", "-q", "-m", message, main, cwd=tree, check=False).returncode != 0:
                raise SystemExit(f"main's text conflicts with the candidate: resolve the merge in {tree}, commit it and run push again")
        head = git("rev-parse", "HEAD", cwd=tree)
        # The gated tree may gain text only, from the merge or from the integrator's hand after a conflict.
        if git("merge-base", "--is-ancestor", commit, head, cwd=tree, check=False).returncode != 0:
            raise SystemExit(f"{tree} no longer holds the candidate {commit[:12]}: build a new one")
        code = [path for path in git("diff", "--name-only", commit, head, cwd=tree).splitlines() if not text_only(path)]
        if code:
            raise SystemExit(f"the merge in {tree} changes code ({', '.join(code[:5])}): build and gate a new candidate")
        commit = head
    git("push", "origin", f"{commit}:refs/heads/main")
    state["landed_as"] = commit
    state_path().write_text(json.dumps(state, indent=1))
    print(f"main = {commit[:12]}: " + ", ".join(e["branch"] for e in state["landed"]))


def cmd_announce(args):
    state = load_state()
    landed_as = state.get("landed_as")
    extra = dict(item.split("=", 1) for item in args.ret)
    # Without a landing only the requests named here are returned: the others stay queued for the next round.
    lines = [f"**Landed:** `{e['request']}` — `{e['branch']}` {e['commit'][:12]}" for e in state["landed"]] if landed_as else []
    lines += [f"**Returned:** `{e['request']}` — {e['why']}" for e in state["returned"]]
    lines += [f"**Returned:** `{name}` — {why}" for name, why in extra.items()]
    lines += [f"**Superseded:** `{name}` — a newer request of its branch" for name in state["superseded"]]
    if not lines:
        raise SystemExit("nothing to announce")
    now = datetime.datetime.now(datetime.timezone.utc)
    name = f"LANDING/{now:%Y-%m-%dT%H%M}Z-{TRACK}-{'landed' if landed_as else 'round'}-{state['candidate'][:8]}.md"
    head = (f"# {'Landed in `main` ' + landed_as[:12] if landed_as else 'A landing round without a landing'}\n\n"
            f"**From:** {TRACK} · **To:** ALL · **Written:** {now:%Y-%m-%d %H:%M} UTC · **Concerns:** the landing queue, "
            f"candidate {state['candidate'][:12]} on `main` {state['base'][:12]}\n\n")
    msgs = worktree("messages", f"origin/{MESSAGES}")
    git("fetch", "-q", "origin", MESSAGES, cwd=msgs)
    git("checkout", "-q", "--detach", f"origin/{MESSAGES}", cwd=msgs)
    path = msgs / name
    path.parent.mkdir(exist_ok=True)
    path.write_text(head + "\n".join(lines) + "\n")
    git("add", name, cwd=msgs)
    git("commit", "-q", "-m", f"{TRACK} → ALL: landing round {state['candidate'][:12]}" + (f"\n\n{TRAILER}" if TRAILER else ""), cwd=msgs)
    for _ in range(3):
        if git("push", "-q", "origin", f"HEAD:{MESSAGES}", cwd=msgs, check=False).returncode == 0:
            print(f"announced in {name}")
            return
        git("fetch", "-q", "origin", MESSAGES, cwd=msgs)
        git("rebase", "-q", f"origin/{MESSAGES}", cwd=msgs)  # only this unpushed message moves; never forced
    raise SystemExit("could not push the answer")


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("queue")
    candidate = sub.add_parser("candidate")
    candidate.add_argument("--exclude", action="append", default=[], metavar="BRANCH")
    candidate.add_argument("--push", action="store_true")
    gate = sub.add_parser("gate")
    gate.add_argument("--preset", choices=sorted(PRESETS), default="full")
    gate.add_argument("--lanes", help="only these lanes, comma-separated (i9, rz, rz-2, pcu)")
    gate.add_argument("--rerun-lane", help="where a failed group runs again (default: where it failed)")
    gate.add_argument("--tag")
    gate.add_argument("--dry-run", action="store_true")
    sub.add_parser("push")
    announce = sub.add_parser("announce")
    announce.add_argument("--return", dest="ret", action="append", default=[], metavar="REQUEST=REASON")
    args = parser.parse_args(argv)
    {"queue": cmd_queue, "candidate": cmd_candidate, "gate": cmd_gate, "push": cmd_push, "announce": cmd_announce}[args.command](args)


if __name__ == "__main__":
    main()
