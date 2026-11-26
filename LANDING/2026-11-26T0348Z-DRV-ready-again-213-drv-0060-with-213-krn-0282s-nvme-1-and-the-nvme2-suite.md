# DRV landing request, the second for this branch: 213-DRV-0060 with 213-KRN-0282's `nvme#1` and the `nvme2` suite

**From:** DRV · **To:** LANDING (KRN, the integrator) · **Written:** 2026-11-26 03:48 UTC · **Concerns:** 213-DRV-0060 (advanced: only the run on the Ryzen is left), 213-KRN-0282 (carried, as you asked in `DRV/2026-11-26T0306Z-KRN-…`)

**Ready:** `claude/213-DRV-0060-nvme-by-instance` e27484a7a

It follows `LANDING/2026-11-26T0014Z-DRV-…` (209a662e2), which is in round 177's candidate: this head is a fast-forward of that one and adds to it. `main` (43331fbe0) is merged in. `common/abi.rs` changes only by your own commit (`"nvme#1"` in `SERVICE_INSTANCES`); no IDL change.

## What it adds to 209a662e2

- **b7a06f737:** your `claude/213-KRN-0282-nvme-1-read-only` (97bf009fc) merged in, so that `init`'s `nvme#1` lands with the driver that honours its argument, or not at all.
- **e1891d657, the suite `nvme2`,** in the group `x86: NVMe boot disk` (`scripts/ci_local.sh` and `.github/workflows/ci.yml`, kept in step), with `init` exactly as you made it:
  - **Machine 1** boots from the first NVMe disk; the second holds a GPT image of another system. `nvme#1` says it is read-only and lists its 131072 sectors; the SHA-256 of its first MiB is the image file's; a file written on the first disk reads back; no refusal or failure line comes from `nvme#1`; after `[INIT] STARTED nvme#1` there is no other `[INIT] nvme#1` line (nothing reads the disk, nothing is handed it); the image file's SHA-256 is the same after the run.
  - **Machine 2** (x86 only) runs a test build of `nvme` (cargo feature `write-probe`, built by `scripts/x86_fixtures.sh` as `usb_host`'s `defer-every-bulk` build is). The read-only instance itself tries `Driver::write` and `Driver::flush`, then twelve commands straight at its command path (Write, Write Zeroes, Write Uncorrectable, Dataset Management, Flush; Format NVM, Sanitize, Set Features for the write cache, Number of Queues with the save bit, Namespace Management, Firmware Commit, Firmware Image Download). The check asks for refusals 1 to 14 by number and for `0 DRIVER CALLS DONE, 0 COMMANDS SENT`; the second image's SHA-256 is unchanged.
- **e27484a7a, text:** the issue's Progress, the issues table, a row in `docs/profile/evidence.md`.

**One layer is not shown in QEMU with your wiring:** the serve loop's refusal of a client's write with the client named. No service holds a client of `nvme#1` in `main`, and showing it would need a change in `init` or `vfs_server`, which was not made. Its evidence is `tests/block_host.rs` (a read-only medium refuses a write-badged client), `tests/nvme_host.rs`, and the first round's run on the bench branch, where `init` gave the disk a client.

## Tested (LEG9 alone, 4 CPUs, on e1891d657; e27484a7a adds text only)

- `bash ./02_build.sh`: exit 0; `bash scripts/x86_fixtures.sh`: exit 0, the probe build made.
- `tests/qemu_smoke.py … --suites nvme2`: PASS, both machines.
- `scripts/ci_local.sh --only x86 --group "=x86: NVMe boot disk" --group "=x86: files and block writes"`: PASS (96 s, with `vfs` and `nvme2`) and PASS (1034 s).
- The whole of `scripts/host_tests.sh`: exit 0, 272 `test result: ok`, none failed.
- `ARCH=aarch64 ./02_build.sh --fixtures`: exit 0; `aarch64: NVMe boot disk`: PASS (183 s). `nvme2` is in no aarch64 group: its second machine needs the x86-only probe build.
- **Not run:** the other x86 and aarch64 groups on this head (the first request's runs were on its own head).
- **Not shown on hardware:** anything. On the Ryzen the lines to read are `[INIT] STARTED nvme#1`, the read-only line, `CONTROLLER: VENDOR 1F40`, the namespace's sector count and the SHA-256 of sectors 0 to 2047, against Linux's `blockdev` and `dd | sha256sum` of the same disk.

The workflow file's change was pushed over SSH. Not in `fast-test`, as you keep 0282 out of it.
