#!/usr/bin/env python3
"""Validate and reproduce the unchanged NRL reference in a separate directory."""
import argparse
import hashlib
import importlib.metadata
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import zipfile

SHA256 = "5e27aab03013e16ac447f12e7093b39ca36167fb20b9b74368661ea425a38b25"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--work-dir", type=Path, required=True, help="New extraction directory")
    parser.add_argument("--report", type=Path, required=True, help="New report file")
    parser.add_argument("--tests", action="store_true", help="Run pytest using this interpreter")
    args = parser.parse_args()
    archive = Path(__file__).resolve().parents[1] / "reference/qwen-spline-lab-v0.1.0.zip"
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    if digest != SHA256:
        parser.error("Reference archive SHA-256 mismatch")
    if args.work_dir.exists() or args.report.exists():
        parser.error("Work directory and report must be new; existing files are not overwritten")
    with zipfile.ZipFile(archive) as bundle:
        names = bundle.namelist()
        if len(names) != len(set(names)):
            parser.error("Duplicate archive entries")
        for item in bundle.infolist():
            path = Path(item.filename)
            if path.is_absolute() or ".." in path.parts or path.parts[0] != "qwen-spline-lab":
                parser.error("Invalid archive path")
            if item.file_size > 2_000_000 or item.is_dir():
                parser.error("Unexpected archive entry")
        manifest = bundle.read("qwen-spline-lab/MANIFEST.sha256").decode()
        expected = set()
        for line in manifest.splitlines():
            checksum, name = line.split("  ", 1)
            full = "qwen-spline-lab/" + name
            if full in expected or hashlib.sha256(bundle.read(full)).hexdigest() != checksum:
                parser.error("Internal manifest mismatch: " + name)
            expected.add(full)
        if expected | {"qwen-spline-lab/MANIFEST.sha256"} != set(names):
            parser.error("Internal manifest does not cover the archive")
        args.work_dir.mkdir(parents=True)
        bundle.extractall(args.work_dir)
    result = {
        "schema": "nrl.reference-report/0", "archive_sha256": digest,
        "manifest_files_verified": len(expected), "python": platform.python_version(),
        "platform": platform.platform(), "test_run": None,
        "packages": {d.metadata["Name"]: d.version for d in importlib.metadata.distributions()},
        "historical_protocol": "2026-10-22: 59 passed, 1 skipped; not this run",
    }
    if args.tests:
        env = dict(os.environ, HF_HUB_OFFLINE="1", TRANSFORMERS_OFFLINE="1",
                   OMP_NUM_THREADS="1", MKL_NUM_THREADS="1", PYTHONHASHSEED="0")
        command = [sys.executable, "-m", "pytest", "-q", "-rA"]
        run = subprocess.run(command, cwd=args.work_dir / "qwen-spline-lab",
                             env=env, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        result["test_run"] = {"command": ["python", *command[1:]], "exit_code": run.returncode,
                              "output": run.stdout, "network_policy": "HF offline flags; no model download requested"}
    with args.report.open("x") as out:
        json.dump(result, out, indent=2, sort_keys=True)
        out.write("\n")
    print(json.dumps({"report": str(args.report), "archive_sha256": digest,
                      "tests_exit": result["test_run"]["exit_code"] if args.tests else None}))
    return result["test_run"]["exit_code"] if args.tests else 0


if __name__ == "__main__":
    raise SystemExit(main())
