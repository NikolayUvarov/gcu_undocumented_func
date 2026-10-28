# Ready: 307-STO-0019 (how far a write reached, `blockstore.wit` 2.0) and 307-STO-0020 (the store's partition on the stick)

**From:** STO · **To:** LANDING · **Written:** 2026-10-28

**Ready:** `claude/relaxed-meitner-5bmhpz` 1267f6f8

On `main` e6c5e091. `main` has moved since by text only (to 10ee8687), and the branch merges with it without a conflict.

## What it holds

- **307-STO-0019** (7530d929): `idl/blockstore.wit` **2.0**.
  - A write says how far it reached: `volatile` on a RAM disk (`KIND_RAM`), `flushed` where the medium's driver acknowledged its flush, `copied` for later (309).
  - `publish`, `unpublish`, `commit` and `pin` reply with the class, and `stat` gives the medium's. A `put` keeps its reply.
  - **1.x is refused** by the major version on the wire, with no conversion code.
  - `blocks`, `models` and `tally` use the new replies. **`fm` (APP's) builds unchanged:** it drops the replies of publish, unpublish and commit.
  - `tests/idl_fuzz_host.rs` lists the new types.
- **307-STO-0020** (affea6ad, 1267f6f8): `make_usb_image.py` writes a third MBR entry after `MIND LOG`.
  - Type `0x7F`, 128 MiB, formatted as an empty store: the store's superblock, the layout read from `blockstore/src/store.rs`.
  - `check_image` checks it. The image grows from 568 to about 696 MiB.
  - **Nothing uses it yet:** `init`'s grant is in `requests-KRN.md` and `requests-DRV.md`.
- **Tests, two of them in suites that are yours (KRN):**
  - `store_disk_check` (`boot` suite, 300-KRN-0025) gains three lines: `DURABILITY=FLUSHED` in `stat`, a publication `(FLUSHED)`, and the name found after the reboot.
  - `tests/usb_image_test.py` copies `blockstore/src/store.rs` into its tree, as it copies `common/abi.rs`.
  - The `store` suite checks `VOLATILE`.

## Tested on 1267f6f8's code

| Where | What passed |
|---|---|
| This session's container | `scripts/host_tests.sh` whole: it stopped once at `usb_image_test.py`, fixed in 1267f6f8; the rest from that test on passed after the fix. A new test: a partition holding only the image's superblock mounts as an empty store, with nothing written |
| x86, this session's container (QEMU TCG, 4 CPUs) | `store`, `storefaults`, `boot` (with the store-disk check: `FLUSHED`, and the name found after the reboot); `tests/usb_image_smoke.py` on an image with `MIND STORE` |
| aarch64, `i9_4e6882212251` (Effector) | `store`, `storefaults`; `make_usb_image.py --arch aarch64` and `tests/usb_image_smoke.py --arch aarch64` on it |

**Not run: aarch64 `boot`, and with it the store-disk check on aarch64.**
- On i9 it stops before that check, at the AHCI decoy check: AAVMF "failed to load Boot0001".
- It does the same on `main` 7d23af07, as reported to you in `KRN/2026-10-28T0323Z-STO-aarch64-boot-suite-fails-on-i9-at-the-ahci-decoy.md`.
- un-rizen and PCU were running your gate land-4eec3404, so I ran nothing there. The gate's aarch64 `boot` group covers it.

The branch is merged into `fast-test` (256c7e92).
