"""scripts/build_jobs.sh's run_jobs with stand-in jobs (000-KRN-0020): a failing job is named with its errors at the end,
the build fails, and the jobs still running finish with complete logs."""
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]


def run_jobs(jobs, limit):
    with tempfile.TemporaryDirectory() as work:
        lines = "".join(f"{name}\t{work}\t{command}\n" for name, command in jobs)
        result = subprocess.run(["bash", "-c", f'source "{ROOT}/scripts/build_jobs.sh" && run_jobs "{work}/logs" {limit}'],
                                input=lines, capture_output=True, text=True, timeout=60)
        logs = {p.stem: p.read_text() for p in Path(work, "logs").glob("*.log")}
    return result, logs


class RunJobs(unittest.TestCase):
    def test_a_failure_is_named_with_its_errors_and_the_others_complete(self):
        result, logs = run_jobs([
            ("broken", "echo 'error[E0425]: cannot find value `x` in this scope'; echo ' --> src/main.rs:3:5'; exit 101"),
            ("slow", "sleep 1; echo first; sleep 1; echo last"),
            ("quick", "echo built"),
        ], limit=2)
        self.assertEqual(result.returncode, 1, result.stdout)
        self.assertIn("    ok      slow", result.stdout)
        self.assertIn("    ok      quick", result.stdout)
        self.assertRegex(result.stdout, r"    FAILED  broken \(\d+ s\): .*/broken\.log")
        tail = result.stdout[result.stdout.index("=== broken failed"):]
        self.assertIn("error[E0425]: cannot find value `x` in this scope", tail)
        self.assertIn("src/main.rs:3:5", tail)
        self.assertEqual(logs["slow"], "first\nlast\n")
        self.assertEqual(logs["quick"], "built\n")

    def test_every_job_ok_returns_zero(self):
        result, logs = run_jobs([(f"crate{n}", f"echo {n}") for n in range(5)], limit=2)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(sorted(logs), [f"crate{n}" for n in range(5)])
        self.assertNotIn("FAILED", result.stdout)

    def test_a_failure_without_compiler_errors_shows_the_logs_end(self):
        result, _ = run_jobs([("linker", "for i in $(seq 40); do echo line $i; done; exit 1")], limit=1)
        self.assertEqual(result.returncode, 1)
        self.assertIn("line 40", result.stdout)
        self.assertNotIn("line 5\n", result.stdout)


if __name__ == "__main__":
    unittest.main()
