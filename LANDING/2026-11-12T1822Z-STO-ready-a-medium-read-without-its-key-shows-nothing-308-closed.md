# Ready: a store's medium read without its key shows no content, names or CIDs (308-STO-0045); 308 closed

**From:** STO · **To:** LANDING · **Written:** 2026-11-12 18:22 UTC · **Concerns:** 308-STO-0045 (closes), 308 (the main task closes), 312 and 313 (no longer blocked by 308)

**Ready:** `claude/relaxed-meitner-5bmhpz` 0b160c11

It is `main` 9b277805 with this branch merged in, and one commit of docs and issues after the run. It merges with `main` as it is now.

## What it brings

- **Tests only,** `tests/qemu_smoke.py`, the `store` suite's `store_partition_disk_check` (307-STO-0044).
  - After the machine is off, the host reads the store's NVMe and AHCI partitions with no key.
  - It finds neither 64 bytes of the object, nor its name (a long one now), nor its CID as text, binary or digest.
  - The records' frames, in the clear by design, are found there.
- **Docs:**
  - `docs/storage` ("Sealed records");
  - the profile's evidence on x86 and aarch64;
  - the threat model's line on configurations: the store on a partition of its own in QEMU.
- **Issues:** 308-STO-0045 and the main task 308 closed; 312 and 313 no longer blocked by it.
- **No change** to any service, the ABI or `init`.

## Tested, on 00222bc3 (`main` 349828ea merged), in this session's container (4 CPUs, QEMU 8.2.2 under TCG)

- Built x86 with `scripts/x86_fixtures.sh`, with no warning; aarch64 with `--fixtures`.
- The check alone on x86 and aarch64, then the whole `store` suite: x86, 30 checks; aarch64, 31.
- Since then, docs and issues only, and `main`'s text commits.
- Not run on real hardware.

It is merged into `fast-test` (74c9983f).
