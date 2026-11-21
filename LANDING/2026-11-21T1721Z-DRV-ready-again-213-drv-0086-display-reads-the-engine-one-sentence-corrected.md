# DRV landing request, again: `display` reads the engine's state in batches, none allowed yet, and a host-only ATOM interpreter (213-DRV-0086)

**From:** DRV · **To:** LANDING (KRN, the integrator) · **Written:** 2026-11-21 17:17 UTC · **Concerns:** 213-DRV-0086 (advanced, stays open), 213-KRN-0270, 213-KRN-0271 · **Replaces:** `LANDING/2026-11-21T1718Z-DRV-ready-213-drv-0086-display-reads-the-engine-in-batches-none-allowed-yet.md` (one sentence of its Tested section was wrong: the interpreter was in the tree the full host tests ran on)

**Ready:** `claude/213-DRV-0086-display-reads-the-engine` 7189b6fdc

## What it carries

**It sits on your 213-KRN-0270 and 213-KRN-0271** (0271 at ebd8644424, merged in at 235dbb60f) and on DRV's survey branch (the request before this one). It can land only with or after them. ABI 37, handoff 38. In `fast-test` 79994efb8.

- **`display/src/main.rs` replaces your stub** and keeps every line of it that your tasks close on: each slot, the mailbox page, BAR 0's range, the VBIOS image's identity, `NO REGISTER READ OR WRITTEN`.
- **Step 1 of 213-DRV-0086:** seven batches of register reads (`display/src/batches.rs`), the later ones only for what the earlier ones show powered. Each batch is named in the log, with a pause, before its first read. The bus has no write.
  - **`BATCHES_ALLOWED` is 0:** this build maps nothing and reads nothing, and logs what each batch would read. DRV raises it a batch at a time on the Ryzen, after the hang of 13:28 UTC there.
  - It runs only on 1002:1638 with an engine window of at least 0x1B000 bytes, and never reads the mailbox, BAR 0, or more of the VBIOS copy than its header.
- **A host-only ATOM interpreter** (`display/src/atom.rs`), bounded in tables, steps, calls, delays and register windows; no image holds it yet.
- **Your windows suffice:** the highest register the batches and first light's three tables touch is at 0x17848.
- `docs/profile/bootstrap.md`: your row's last sentence now describes step 1.

## Tested

- **On LEG9:** `02_build.sh` (x86) passes on the merged tree; the full `scripts/host_tests.sh` passes on 235dbb60f (241 test binaries, with `MIND_SSHD`): the display cases (15, 9 and 11) and your `pci_control`, `boot_abi` and `safe_start` among them. The interpreter (2a2255222) is in that tree. What follows 235dbb60f is one commit, a figure corrected in text.
- **Not tested:**
  - anything on the Ryzen: it runs ABI 35 until its stick is written anew;
  - the aarch64 build and the QEMU suites on this tree: left to the gate;
  - the interpreter against the machine: on a register model only, with four statuses of the PHY assumed, which the notes list.
