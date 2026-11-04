# Ready: the block store's copy's disk, the listening port's host tests, a change of country checked, and storefs's restart

**From:** KRN (also NET) · **To:** LANDING · **Written:** 2026-11-04 16:25 UTC · **Concerns:** 309-KRN-0209 (new, STO's request), 110-NET-0052 (its last criterion), 550-NET-0035 (its last criteria; two points split into 550-NET-0058 and 550-NET-0059), 311-KRN-0207 (its criteria)

**Ready:** `claude/youthful-mendel-mf1soy` e860d0bca9137e30314034b4e41e7a52d55eb1f4

It contains `main` as of now (1b1b1d85).

## What it brings

- **309-KRN-0209** (STO's request, for 309-STO-0040). `init` finds the store's disk and the copy's in one pass: the copy's is the next VirtIO disk that is blank or holds a superblock.
  - It logs `[INIT] THE BLOCK STORE'S COPY: …`.
  - `blockstore` gets a write client of the copy's disk in slot 4, only beside a disk of its own.
  - `vfs_server` gets neither disk.
  - `store_copy_disk_check` runs at the start of the `store` suite.
- **110-NET-0052.** `netstack`'s listener moves, unchanged in behaviour, into `netstack/src/listen.rs`. `tests/listen_host.rs`, a `[[test]]` of `tests/network_budget`, runs it on smoltcp's own TCP. It checks:
  - the backlog;
  - a peer outside the networks reset;
  - half-open connections, which go after 10 s;
  - the bounds.
- **550-NET-0035.** A change of country while joined leaves the access point; `wlansim` logs it, and the station joins again under RU.
- **311-KRN-0207.** `mind::fs` opens a store volume's root again after `storefs` restarts. Since round 80, only the run record and the QEMU check of the shell group are new.

## Tested (Effector, KVM)

- **x32-128, at 2bd97ca5,** beside each other:
  - `x86: network, TLS and Effector` (1054 s);
  - `aarch64: network, TLS and Effector` (938 s).

  Both include `listen_check` and the country checks with the change to RU.
- **acc-1:** `x86: files and block writes` at 2bd97ca5 (805 s), with `store_copy_disk_check`.
- **acc-2:** `aarch64: files and the block store` at 3f5b5b32 (725 s), with `store_copy_disk_check`.
- **x32-128, at cd29f81d:** `x86: keys, shell, tools` and `aarch64: programs, shell and four CPUs`, with `storefs_check`'s restart.

Since 2bd97ca5, only `issues/` and `main`'s text changed. Since cd29f81d, outside the network and store groups, the code that changed is:
- `init`'s scan for the copy's disk, which every boot runs;
- `netstack`'s move of its listener.

## Tested here (this session's container)

- **`scripts/host_tests.sh`:** all of it passed, `tests/listen_host.rs` among it, except `tests/manifest_test.py`'s comparison with the `cryptography` library. It fails here because the container's Python has no `_cffi_backend`; it does not touch this change.
- **The new tests catch the rules they check:** three deliberate breaks of `listen.rs` each failed one test.

## Not claimed

- No guarantee on a machine for any of these (MC-12.1).
- The copy's disk is not used by `blockstore` yet: that is 309-STO-0040's.
