# KRN ready: the gateway checks wait up to 60 s for `dmesg`, and a gate resumes under a later push

**Ready:** `claude/youthful-mendel-mf1soy` c4e7b561

`main` 60c27a9a is merged in, with no conflict.

## What it lands

- **e340ab6b, `tests/qemu_smoke.py` (750-KRN-0121's check).**
  - **The failure.** Round 41's group "aarch64: files, network and TLS" failed once on `un-rizen` (it passed its rerun) in `gateway_stub_check`.
    - After `svc restart effector_gw`, `dmesg -s effector_gw` prints both starts' lines, about 11 KB.
    - The console under TCG had printed 8.9 KB when `VM.command`'s 8 s ran out.
  - **The fix.** The checks that read the gateway's log, and `init`'s after the restart, wait up to 60 s for the prompt.
- **2ac6dac7, `scripts/land.py`.**
  - **The problem.** Round 41's candidate was a request's own commit, published on its branch. A fix pushed to that branch meanwhile kept the resumed gate from starting ("not on origin").
  - **The fix.** `gate` fetches the branch and checks that it holds the candidate, since the machines check it out by its hash.

## Tested (this container)

- **The gateway check on aarch64 (QEMU, 4 CPUs):** `gateway_stub_check` alone passed.
  - Each `dmesg` took 0.5 to 1.2 s here, for up to 11 148 bytes.
  - So on `un-rizen` the console was about 15 times slower when it failed.
- **`tests/land_test.py`:** all passed.
- **The resumed gate of round 41 itself** ran with this `land.py`.
