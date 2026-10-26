# Ready: the link's mark, effector485's leases, the write script's keeps, and several machines on one RS-485 line

**From:** LNK · **To:** LANDING · **Written:** 2026-10-26 08:20 UTC · **Concerns:** 800-LNK-0019 to 800-LNK-0026 · **Replaces:** nothing pending (LNK's last request, 316bbf15, landed in 84053da5)

**Ready:** `claude/LNK-link` ee66472883edb6fad09a3a25b7fca50a1653df4b

## What it brings

Everything on the branch since 316bbf15, the last landed:

- **In an image:** `link.elf`.
  - It lights its mark on the screen through `display.wit` 1.2 `remote` (800-LNK-0020, on KRN's 800-KRN-0156 in `main`).
  - It reads `address=<n>` from `link.conf` and then answers only frames to that address (800-LNK-0026). Without the word, nothing changes.
- **`linkproto`:** frame version 2 with the slave's address (1 to 247). A slave without an address answers version 1 only, as before. `Line` gains `claim` and `release`, no-ops by default.
- **Host tools** (`tools/mindlink`):
  - `mindlink --address`;
  - `effector485`:
    - keep-warm POLL (0020);
    - leases (0021);
    - the hang after a cancel fixed (0022, closed);
    - several machines on one cable, `--addresses` (0026).
- **`scripts/build_and_write.sh`:**
  - `--mode=…,link-console` (0019);
  - `--keep-effector` and its wait for the old disk (0023);
  - `--keep-settings` (0024);
  - the new script re-run after the pull (0025);
  - `--link-address=<n>` (0026).
- **`tests/qemu_smoke.py`:** the `link` suite checks the mark (0020) and a second boot with `address=5` (0026).
- **Docs:** `docs/link/*` (EN and RU), `docs/write-disk*`, the issues.

No ABI change. No IDL change.

## `main` in it

fb821bbd, merged in fc7457c8.

## What was run on the branch

- **`linkproto`:** 21 host tests passed, a simulated shared line with three addressed slaves and one without among them.
- **`tools/mindlink`:** every test passed on Linux:
  - `mindlink`'s and `effector485`'s, two machines on one simulated cable among them;
  - `tests/bus.rs`, four `mindlink serve` on one hub.
- **Builds:**
  - `effector485.exe` for Windows (x86_64-pc-windows-gnu) built;
  - `./02_build.sh` passed.
- **QEMU x86:**
  - `link` passed on 1f7fcf5a: the mark, then `address=5`;
  - `link` and `linkupdate` passed again on ee664728 (2026-10-26): release 6 through the link, the refused channels, the slot's own applications; the `link` suite with its `address=5` boot.
- **`build_and_write.sh`:** only its parsing was run. `--link-address` of 0, 248 or a non-number is refused. Its earlier keeps ran on the maintainer's writes, as 800-LNK-0023 and 0024 record.
- **Hardware:** for 0026, none yet. The maintainer is buying the converters for the shared pair. The MacBook stays alone on COM22, at no address, where nothing changes.
