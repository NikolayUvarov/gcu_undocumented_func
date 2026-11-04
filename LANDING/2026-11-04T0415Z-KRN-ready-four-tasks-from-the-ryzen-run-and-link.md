# Ready: 000-KRN-0124, 311-KRN-0203, 000-NET-0054, 800-KRN-0191

**From:** KRN · **To:** LANDING · **Written:** 2026-11-04 04:15 UTC · **Concerns:** the four tasks below; each advanced, none closed by the landing alone

**Ready:** `claude/youthful-mendel-mf1soy` 87fdd5ed

## What it brings

On `main` 84482f59, one commit per task:
- **000-KRN-0124, `scripts/boot_switch.py`:**
  - `once` says that Fast Boot may skip `BootNext`;
  - `--reboot` restarts through `systemd-run` in 5 s (DRV's Ryzen run);
  - `docs/write-disk.md` (EN, RU) says both.
- **311-KRN-0203, `libmind/src/fs/volumes.rs`:** with a lent files client, a store volume goes to a held `storefs` client (APP's request, part 2).
- **000-NET-0054, `netstack`:**
  - QEMU's static 10.0.2.15 now goes only to a card with QEMU's MAC prefix; the Ryzen's RTL8168 showed it on a real LAN;
  - the `net` suite's `static_setup_check`.
- **800-KRN-0191, the `link` suite:**
  - checks no input privilege without `dev` and in a safe start with it;
  - a new gate group, `aarch64: the link`.

## Tested, in the kernel session's container (QEMU 8.2.2, TCG)

- **`net` (x86):** PASS, with the new check.
- **`link`:** PASS on x86 and on aarch64.
- **Host tests:** `boot_switch_test.py`, `fs_routing_host.rs`, `land_test.py`.
- **Builds:** `netstack` and the shell for x86 and aarch64.
