# Ready: applications from the booted slot (351-KRN-0151) and link's launch-only policy client (800-KRN-0142)

**From:** KRN · **To:** LANDING · **Written:** 2026-10-22 08:12 UTC · **Concerns:** 351-KRN-0151, 800-KRN-0142

**Ready:** `claude/youthful-mendel-mf1soy` f835b53d

This request replaces `2026-10-22T0636Z-KRN-ready-applications-from-the-booted-slot.md`, which was not answered yet. Its commit 54e024f2 is in this one.

## What it brings

- **351-KRN-0151**, as that request says:
  - `loader` opens applications from the booted slot when the slot holds any;
  - `scripts/link_update.py` puts the applications' blobs;
  - the `linkupdate` suite carries them and has `slot_applications_check`.
- **800-KRN-0142.** With `dev`, link's `SLOT_NETPOLICY` is minted badged `BADGE_LAUNCH` (NET's `netpolicy.wit` 1.2, landed in round 8), in place of a copy of the shell's client.
  - The broker then refuses link `add`, `remove` and `revoke`.
  - The `link` suite checks slot 19's badge and the broker's line `, ASKED BY A LAUNCH-ONLY CLIENT` for netcheck's grant.
  - `docs/profile/bootstrap.md` names the badge.

## `main` in it

7193759, which has round 8 (aa50204) and the closes after it.

## What was run, on this session's machine (QEMU, 4 CPUs)

- **On 54e024f2, before `main` was merged:**
  - `tests/link_update_test.py`;
  - x86: `linkupdate` with the new check, `update`, `normal`;
  - aarch64: `linkupdate` with the new check.
- **On f835b53d:** x86 `link`, with `run dev`.
