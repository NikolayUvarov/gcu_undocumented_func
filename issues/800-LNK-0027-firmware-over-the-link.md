# 800-LNK-0027 — Proprietary firmware onto the machine over the link, so that no disk is written again for it

**Type:** service, tool · **Owner:** `LNK` · **Priority:** P1 · **Status:** open (2026-10-27) · **Blocked by:** `KRN`: in developer mode, `link` gets a writable scope of `data/firmware/` (asked on `internal-dev-messages`, 2026-10-27) · **Main task:** [800](800-mind-link.md) · **Constitution:** MC-2.3, MC-10.2, MC-12.3

## Problem

The maintainer (2026-10-27): `effector485` was built to close the question of writing the disk again. A machine with slots should be updated over the cable alone.

- **What goes over the cable today:**
  - the kernel, the boot services and the applications, as a release into the other slot (`scripts/link_update.py`). Each slot carries its own loader (351-UPD-0010), so a new handoff goes too;
  - a service replaced until the next boot (`replace`);
  - files in `data/link/`.
- **What does not: the proprietary firmware** in `data/firmware/` (AGENTS.md, section 3).
  - **The drivers read it there:** `bcm_wifi`, `rtw89_wifi`, `rtw88_wifi`.
  - **`link` reaches only `data/link/`**, init's scope for it (800-KRN-0128).
  - **No way through the shell:** its client for the link refuses `mv` and `mkdir` (`shell/src/clients.rs`).
  - So a driver whose firmware is new needs the disk written again with `scripts/proprietary.sh copy`.
- **The case:** the DEXP's `rtw88_wifi` (550-DRV-0042). Its firmware `data/firmware/rtw88/rtw8821c_fw.bin` was new, and the stick was written again for it on 2026-10-27.

## Plan

1. **`KRN`: the grant.** In developer mode (`dev` in `link.conf`), `init` gives `link` a second scope, writable, of `data/firmware/`, in `SLOT_FILE`, and makes the directory when it is missing.
   - No new authority in kind: `dev` already lets the cable's holder start any service from an unverified image (`replace`).
   - Each driver checks its firmware by size and SHA-256 before use, so a file from the cable is held to the same pin as one from the disk.
   - Without `dev`, `link` reaches `data/link/` alone, as before.
2. **`link`: the names.**
   - Names under `firmware/` go to that scope: `put`, `get`, `ls`, `stat`, `rm`, `mkdir`, `rmdir`, all logged as `[LINK] … firmware/…`.
   - Without the scope, `link` refuses them as unsupported.
   - A move between the two scopes is refused, since each is its own client; a put goes straight to its place.
3. **The tools.**
   - `scripts/proprietary.sh link` puts every fetched file into `firmware/` over the link, through `mindlink` or `effector485`. A file the other end already holds is skipped by its SHA-256.
   - `scripts/link_update.py --firmware` does the same before a release, for a release whose driver needs a new file.
4. **The docs:**
   - `docs/link/mindlink.md` and `effector485.md` (both languages);
   - THIRD_PARTY.md and AGENTS.md's rule on proprietary files: copied after the write *or over the link* to a machine of the maintainer's own.

   AGENTS.md is the maintainer's text: only its change is proposed.
5. **`effector485 help`** names `replace` and `shell`, which the daemon serves but its help leaves out.

## Acceptance criteria

- [ ] `link`'s host tests: names under `firmware/` reach the second scope, are refused without it, and a move between the scopes is refused.
- [ ] The `link` QEMU suite: `mindlink put` of a file into `firmware/rtw88/` lands in `data/firmware/rtw88/` with `dev`, and is refused without it.
- [ ] On the DEXP: `proprietary.sh link` puts `rtw8821c_fw.bin` over `effector485`; after a restart `rtw88_wifi` reads it. Evidence for the DEXP only (MC-12.1).

## Related

[800-LNK-0016](800-LNK-0016-replace-a-service-over-the-link.md) (`replace`), [800-LNK-0026](800-LNK-0026-several-machines-on-one-line.md), `550-DRV-0042` (the drivers track).
