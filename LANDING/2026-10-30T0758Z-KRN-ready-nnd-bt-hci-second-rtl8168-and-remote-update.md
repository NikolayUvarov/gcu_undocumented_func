# KRN: nnd and bt_hci boot images (ABI 25, handoff 25), a second rtl8168, remote-update in init

**From:** KRN · **To:** LANDING · **Written:** 2026-10-30 07:58 UTC

**Ready:** `claude/youthful-mendel-mf1soy` 75d3df66f918829d44b80599d26292177055fe30

**What it lands:**
- **262-KRN-0178** (advances; APP's shell part left): `nnd` as a boot service.
  - What it holds: read-only scopes of `data/models/` and `models:` where they exist; its line of `data/memory.conf` at each start (quota, `--memory`, workers).
  - The shell's client: `SLOT_NN` (41) with the right to badge, revoked before each new shell, and `REQUEST_NN`.
  - The kernel gives a task the online CPUs in `BootInfo.cpu_count`.
- **700-KRN-0176:** `bt_hci` back as a boot image, now that 700-DRV-0044 is in `main`.
- **ABI 25 and handoff 25**, shared by both images: 46 images, `SLOT_DYNAMIC` 42, `CAP_SLOTS` 106. A loader of handoff 24 or older is refused by name.
- **211-KRN-0187:** `rtl8168#1` for the DEXP's second port (DRV's request).
- **751-KRN-0185 and 750-UPD-0020:**
  - `init.wit` 1.11: `remote-update` and `link-remote-update`, with their rules and the test-key warning;
  - `update.wit` 1.2: the gateway's client `BADGE_UPDATE_GATEWAY`, whose `fetch`, `apply` and `rollback` run only while `remote-update` counts.
- **000-KRN-0188** (the issue only): typed characters lost on the serial line under load.
- **The landing tool:** it logs each machine's load and each lane's minutes, builds at `nice 10`, and `docs/effector.md` records that i9 keeps one lane.

**Tested:**
- **Host tests:** `boot_abi`, `runtime`, `services`, `svc`, `shell` and `land_test`.
- **QEMU in this container** (x86, 4 CPUs): `nnd` and `modes`, the latter with the new `remote_update_check`.
- **On the machines, through Effector:** c5470bdc on `un-rizen` (both lanes), acc-1, acc-2 and PCU, 10 groups, all passed.
  - x86: boot; keys, shell, tools; services, storage, audio; scheduling, isolation, heap; one CPU; network, TLS and Effector; the updater.
  - aarch64: boot; programs, shell and four CPUs; the updater and the modes.
  - Its first try, on 67ab941e, failed the boot suite's FAT check and `fm /` where `init` made `data/models/` at each boot. c5470bdc makes no directory.
- **Since c5470bdc:** text (issues, docs) and a merge of `main`'s text.
