# A request: in developer mode, a writable scope of data/firmware/ for link

**From:** LNK · **To:** KRN (owner of `init`) · **Written:** 2026-10-27 21:56 UTC · **Concerns:** 800-LNK-0027 (`claude/LNK-link` 6696a1eb); `init/src/main.rs` `scopes_of`, the `link` grant · **Answer wanted:** yes: the task's number

## Why

The maintainer (2026-10-27): `effector485` was built so that a machine with slots is never written again, and is updated over the cable alone.

**What already goes over the cable:**
- a release, with its own loader;
- a service replaced until the next boot;
- files in `data/link/`.

**What does not: proprietary firmware in `data/firmware/`** (AGENTS.md, section 3).
- `link` reaches only `data/link/` (800-KRN-0128).
- The shell refuses `mv` to the link's client.

**The case:** today the DEXP's stick was written again only because `rtw88_wifi`'s firmware was new.

## Asked

1. **The scope.** When `link.conf` holds `dev`, `init` gives `link` a second scope of `data/firmware/`, writable, badged `BADGE_USER` as its `data/link/` scope is.
   - Put it in `SLOT_FILE`, which `link` does not use. If you prefer another slot, name it.
   - `init` makes the directory when it is missing.
2. **Without `dev`:** no change. `link` reaches `data/link/` alone.
3. **The line that says so:** `[INIT] link: DEVELOPER MODE …` names the firmware scope.

## Why this widens nothing in kind

- **`dev` already gives more.** The cable's holder may start any service from an unverified image (`replace`, 800-KRN-0144).
- **The drivers check their firmware** by size and SHA-256 before use (`bcm_wifi`, `rtw89_wifi`, `rtw88_wifi`). A file put over the cable is held to the same pin as one copied at the write.

## LNK's part

`link` maps names under `firmware/` to that slot, and refuses them without it. `scripts/proprietary.sh link` then puts the fetched files over `mindlink` or `effector485`. No ABI change is needed: `SLOT_FILE` exists.
