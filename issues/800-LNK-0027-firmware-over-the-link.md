# 800-LNK-0027 — Proprietary firmware onto the machine over the link, so that no disk is written again for it

**Type:** service, tool · **Owner:** `LNK` · **Priority:** P1 · **Status:** in progress (2026-10-27) · **Blocked by:** — (`KRN`'s 800-KRN-0173, merged into this branch) · **Main task:** [800](800-mind-link.md) · **Constitution:** MC-2.3, MC-10.2, MC-12.3

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

## Progress

- **2026-10-27, `KRN`:** 800-KRN-0173 (85b39c18). With `dev`, init gives `link` a writable scope of `data/firmware/` in `SLOT_FILE` and names it in its line. This branch merges the kernel track's branch for it (c881d82f).
- **2026-10-27, `link`** (660cbf0d):
  - **The name rules** are in `link/src/names.rs`, host-tested in `tests/link_names_host.rs`. Names under `firmware/` go to that client; `Store` switches the client per request.
  - **A put to `firmware/`** has its part file in `data/firmware/`, so the finished file is renamed within one client.
  - **A get or a put** keeps its client, since a handle answers only through the client it was opened with.
  - **The top's `ls`** lists `firmware/` last.
  - **Refused:** a move between the places, a change of `firmware` itself, and `run` or `replace` of a name there. Without the grant such names are unsupported.
  - **The developer mode's line** says `FIRMWARE (data/firmware/ AS firmware/)`.
  - **The `link` QEMU suite** puts, reads, lists and removes a file in `firmware/test/`, sees it in `data/firmware/test/`, and checks the refusals.
- **2026-10-27, the tools** (9b1cb0b3):
  - `scripts/proprietary.sh link PORT` puts through `mindlink`, and `link effector485` through the daemon;
  - `effector485 help` names `shell` and `replace`;
  - `mindlink.md` (both languages) and THIRD_PARTY.md are updated.
- **2026-10-27, on the DEXP:**
  - a release with this branch's `link` and KRN's `init` went over the cable (slot B, version 202610272251, confirmed);
  - `link` logs `FIRMWARE (data/firmware/ AS firmware/)`;
  - through `effector485`, `list firmware` shows `b43/`, `rtw88/` and `rtw89/`, and `stat firmware/rtw88/rtw8821c_fw.bin` gives 139,472 bytes and the pinned SHA-256 `2ef409bc…f3f0`.
- **2026-10-28, the first proprietary files over the cable:**
  - `scripts/proprietary.sh link effector485` (EFFECTOR485 the daemon's program) put the RTL8821C Bluetooth controller's files for 700-BLT-0001: `rtl8821c_fw.bin`, `rtl8821c_config.bin` and their licence;
  - they went into `firmware/rtl_bt/`, one operation of the daemon each, under a reservation;
  - `stat` reads back 54,636 and 10 bytes with their pinned SHA-256;
  - no disk was written.
- **Not done:**
  - `link_update.py --firmware` (plan, point 3): `proprietary.sh link` before `link_update.py` does the same;
  - the change to AGENTS.md's rule, which is the maintainer's text: to be proposed.

## Acceptance criteria

- [x] Host tests of the name rules: names under `firmware/` go to `data/firmware/`, a put finishes in its own place, a move between the places and a change of the place itself are refused (`tests/link_names_host.rs`). That such names are unsupported without the grant is in `Store::scope`, not host-tested.
- [x] The `link` QEMU suite (x86, 2026-10-27, with `linkupdate`): `mindlink put` of a file into `firmware/test/` lands in `data/firmware/test/` with `dev`; it comes back by `get`, and a move out of the place is refused.
- [ ] On the DEXP: `proprietary.sh link` puts `rtw8821c_fw.bin` over `effector485`; after a restart `rtw88_wifi` reads it. Evidence for the DEXP only (MC-12.1).

## Related

[800-LNK-0016](800-LNK-0016-replace-a-service-over-the-link.md) (`replace`), [800-LNK-0026](800-LNK-0026-several-machines-on-one-line.md), `550-DRV-0042` (the drivers track).
