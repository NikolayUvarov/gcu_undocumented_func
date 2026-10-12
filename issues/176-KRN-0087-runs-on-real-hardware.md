# 176-KRN-0087 — Two runs on real hardware that collect what debugging needs: `diag/quick.msh` and `diag/full.msh`

**Type:** tools (`diag/`, the image's staging in `02_build.sh`, `scripts/build_aarch64.sh` and `scripts/make_usb_image.py`, tests) · **Owner:** kernel session · **Priority:** P1 · **Status:** in progress (made, tested on the host and in QEMU; the quick run done on the MacBook Pro, the full run left) · **Blocked by:** — · **Roadmap:** Assurance, track A, main task [176](176-test-and-performance-utilities.md) · **Constitution:** MC-12.1, MC-12.2

## Problem

The maintainer (2026-10-11): to collect data from runs on real hardware, two scripts are needed:

- a quick run;
- a full run.

Running either must collect everything debugging needs. The quick run runs only what the current stage of development needs; the full run runs everything.

Until now, each MacBook Pro run was a set of commands typed by hand from the issues' requests (`kbench timer`, `bench files`, …), and what the screen showed was photographed or lost. Only the boot log, the hardware report and the tools' own logs reached the log volume.

## Plan

Two msh scripts on the boot volume, `diag/quick.msh` and `diag/full.msh`, run from the shell. They share the same runner (kept alike in both files, as msh has no includes).

- **The directory.** Each run gets a new one, `log:<kind>NNNN/`, NNNN being this boot's number (the highest `bootNNNN.log`). A second run of the same kind in one boot gets `-2`, `-3` …; without the log volume, `ram:<kind>-001/`.
- **The steps.** Each step is a shell command or a program, run through `capture`:
  - its output goes to a file of its own, `NN-<name>.txt`;
  - a task's several views share one file;
  - output that would not fit one msh string (64 KiB) goes on in `-2`, `-3` ….
- **`summary.txt`.** It names:
  - the build: init's `BUILD:` line, from the system log's ring or, once the ring moved on, from this boot's log file;
  - the date;
  - every step with its result, its time, its file, and the task it serves;
  - the checks only the maintainer can make (by eye, or ones that end the run).
- **If a step stops the machine.** It can still be named:
  - the summary is written before each step with the step running now;
  - each step has a line in the system log, whose file on the log volume outlives the run.
- **A failed step** is recorded with its reason, and the run goes on. A program's exit status other than 0 counts as a failure (`check` exits with 1 when a check fails; its own log has the details).
- **`quick.msh`, this stage's steps:**
  - the state: the clocks, the processors, memory, tasks, services, faults, devices, interrupts, volumes, the log's warnings;
  - the log records of the drivers being worked on: `bcm_wifi` (550-DRV-0023), `usb_host` (211-DRV-0017, 0018) and `audio_gw` (551-DRV-0010);
  - `check kernel clock files sound`;
  - `kbench timer ipc`: 000-KRN-0065, 0083 and 0084;
  - `bench files`: 000-KRN-0078.

  A step goes when its task is done; a new task adds its step, named.
- **`full.msh`, everything:**
  - every view of the state: every `stat` class, physical memory, quotas, endpoints, `fsck`, every volume's listing, the programs, the network, the TPM, the whole system log;
  - for each task: `stat`, `caps`, `pmap` and `logs`;
  - `check`, `kbench` and `bench` with all their groups;
  - the state again after them.
- **The images.** `02_build.sh` and `scripts/build_aarch64.sh` copy `diag/*.msh` to the volume's `diag/`, and `make_usb_image.py` packs them.
- **Not done here.**
  - The scripts are not in the signed boot manifest. A script on the boot volume runs without a question, as `data/*.msh` always did. Whether the manifest should cover them is for the update track (UPD).
  - Full-screen tools (`hw`, `ipc`, `caps`, `top`, `memmap`, `load`) cannot be captured. The hardware report written at every boot (`hwNNNN.txt`) holds what `hw` shows.

## Acceptance criteria

- **The host test `tests/diag_host.rs`** runs both scripts against a shell that answers as the real one does. It checks:
  - the run's directory, with `-2` beside an earlier run;
  - every step's file;
  - the summary with the build, a failed step's reason, and the checks by eye;
  - a task's output split where it would not fit one string;
  - the build found in the boot log once the ring moved on.
- **The QEMU suite `bench` (x86 and aarch64)** runs `diag/quick.msh`:
  - every step passes;
  - each has its file, and the summary names them all;
  - `kbench`'s output is in its file;
  - the shell accepts `diag/full.msh` (`msh --check`).
- **On the MacBook Pro**, both runs complete and the log volume holds their directories.

## Progress

**2026-10-11, the host and QEMU x86 (4 CPUs, TCG).**

- `tests/diag_host.rs` passes.
- The `bench` suite runs `diag/quick.msh`: 19 steps in 58 s, none failed, each in its file under `ram:quick-001` (no log volume there), the build named in the summary.
- A one-off run of `diag/full.msh` in the same machine (not a suite): 67 steps in 94 s; 24 of them were the tasks, each file with `stat`, `caps`, `pmap` and `logs`. Two failed, as they must there: `ls data` and `ls log:`, as that disk has neither.
- The aarch64 image builds with `diag/`, and the USB image holds `diag/quick.msh` and `diag/full.msh`.

**2026-10-12, the MacBook Pro** (`fast-test` c548a85, from the maintainer's `log:quick0001/`):

- `diag/quick.msh` ran 19 steps in 25 s, none failed. Each step had its file, and the summary named the build and the checks by eye.
- What the steps found:
  - `check`: 23 passed;
  - `kbench`: a sleep of 1 ms took 1.99 ms at the median, 10 ms took 10.9 ms; an IPC round trip took 7.12 µs;
  - `bench`: writes of 2 MiB ran at 2.40 MiB/s on `ram:`, 4.69 MiB/s on `data/` and 4.06 MiB/s on `log:` (000-KRN-0078).
- Interrupts on CPU 6, where `usb_host` ran, rose from 1.5 million to 9.3 million over the 25 s run, 22 s of which were `bench files`: about 300 000 a second. They are for 000-KRN-0078 and 000-KRN-0077 to explain.
- **Left:** a run of `diag/full.msh` on the MacBook Pro.

**2026-10-12, the MacBook Pro, the full run** (`fast-test` 3ab50c0, `log:full0002/`):

- 73 steps in 42 s, none failed.
- `check` ran all its groups: 62 passed, 31 skipped (no device), none failed. The three faults in `faults-after` are its security probes, which fault on purpose.
- **The maintainer:** "it hung and hung, then printed many messages at once". The shell drew its screen only between commands, so nothing showed until the script ended.
  - Now each line shows as it is printed: the shell's script host (`requests-APP.md`, a note).
  - Both runs print `DIAG FULL: STARTED` (or `QUICK`) before anything else.
  - The `shell` suite reads the screen while a script sleeps; the host test checks the first line.
- **Left:** the maintainer sees the lines appear during a run.

## Related

[176](176-test-and-performance-utilities.md), [176-KRN-0062](../issues-done/176-KRN-0062-kbench.done), [176-KRN-0063](../issues-done/176-KRN-0063-check.done), [176-KRN-0064](../issues-done/176-KRN-0064-bench.done), [docs/tools/README.md](../docs/tools/README.md) §4.13, [docs/msh.md](../docs/msh.md), [211-KRN-0019](211-KRN-0019-boot-logs-on-the-log-partition.md) (the boot log), [174-KRN-0038](../issues-done/174-KRN-0038-hardware-report.done) (the hardware report).
