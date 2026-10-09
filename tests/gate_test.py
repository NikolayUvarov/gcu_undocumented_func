#!/usr/bin/env python3
"""The gate fails when any one of its compiles, builds or tests fails (175-KRN-0046, audit A06).

The host tests and the x86 test programs run as CI's steps and ci_local.sh's groups run them, with rustc, cargo and
python3 replaced by stand-ins that count their calls. Every call is made to fail in turn, a test binary too; each run
must end nonzero. Nothing is compiled.
"""
from pathlib import Path
import os
import re
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
# A stand-in: counts its call; fails when the count is $STUB_FAIL; rustc's -o output becomes a test binary that does the same.
STUB = r'''#!/bin/bash
n=$(( $(cat "$STUB_DIR/count") + 1 )); echo $n > "$STUB_DIR/count"
[[ $n == "$STUB_FAIL" ]] && { echo "stand-in call $n fails"; exit 42; }
while [[ $# -gt 0 ]]; do
    if [[ $1 == -o && $2 != "$MIND_TEST_OUT"/* ]]; then echo "stand-in told to write $2, outside MIND_TEST_OUT"; exit 99; fi
    if [[ $1 == -o ]]; then mkdir -p "$(dirname "$2")"; printf '#!/bin/sh\nexec "%s/step"\n' "$STUB_DIR" > "$2"; chmod +x "$2"; fi
    shift
done
exit 0
'''
STEP = '''#!/bin/bash
n=$(( $(cat "$STUB_DIR/count") + 1 )); echo $n > "$STUB_DIR/count"
[[ $n == "$STUB_FAIL" ]] && { echo "test binary call $n fails"; exit 1; }
exit 0
'''


def github_step(name):
    """The run: of CI's step `name`, as GitHub runs it (bash -e)."""
    workflow = (ROOT / ".github/workflows/ci.yml").read_text()
    match = re.search(rf"\n      - name: {re.escape(name)}\n(?:        if: .*\n)?        run: (.*)\n", workflow)
    assert match, f"CI has no step {name!r}"
    assert match[1] != "|", f"CI's step {name!r} should be one command"
    return match[1]


def local_group(name):
    """The command of ci_local.sh's group `name`, as run_part evaluates it."""
    script = (ROOT / "scripts/ci_local.sh").read_text()
    match = re.search(rf'"{re.escape(name)}\|([^"]*)"', script)
    assert match, f"ci_local.sh has no group {name!r}"
    return match[1]


def run(command, stubs, fail, out):
    (stubs / "count").write_text("0")
    env = {**os.environ, "PATH": f"{stubs}:{os.environ['PATH']}", "STUB_DIR": str(stubs), "STUB_FAIL": str(fail), "MIND_TEST_OUT": str(out)}
    result = subprocess.run(["bash", "-e", "-c", command], cwd=ROOT, env=env, capture_output=True, text=True)
    return result.returncode, int((stubs / "count").read_text())


def main():
    failures = []
    with tempfile.TemporaryDirectory(prefix="mind-gate-test-") as temporary:
        stubs, out = Path(temporary) / "bin", Path(temporary) / "out"
        stubs.mkdir()
        for tool in ("rustc", "cargo", "python3"):
            (stubs / tool).write_text(STUB)
            (stubs / tool).chmod(0o755)
        (stubs / "step").write_text(STEP)
        (stubs / "step").chmod(0o755)
        entries = [("CI: Host tests", github_step("Host tests")), ("ci_local: host tests", local_group("host tests")),
                   ("CI: Test programs", github_step("Test programs")),
                   ("ci_local: build (x86 test programs)", local_group("build (x86 test programs)"))]
        for label, command in entries:
            status, calls = run(command, stubs, 0, out)
            if status != 0 or calls < 10:
                failures.append(f"{label}: {status} with no call failing, {calls} calls")
                continue
            for fail in range(1, calls + 1):
                status, _ = run(command, stubs, fail, out)
                if status == 0:
                    failures.append(f"{label}: call {fail} of {calls} failed, the run passed")
            print(f"{'FAIL' if any(f.startswith(label) for f in failures) else 'PASS'}: {label}, each of {calls} calls failing in turn")
    if failures:
        sys.exit("FAIL:\n  " + "\n  ".join(failures))


if __name__ == "__main__":
    main()
