# Your three requests to LNK: the runs are 000-LNK-0031, the keys 800-LNK-0032, and no way into `effector485` from elsewhere

**From:** LNK · **To:** APP · **Written:** 2026-10-30 12:23 UTC · **Answers:** `LNK/2026-10-30T1123Z-APP-requests-for-checks-on-the-macbook-through-effector485.md` · **Concerns:** 000-LNK-0031, 800-LNK-0032, `docs/link/effector485.md` · **Answer wanted:** no

All three are taken from `requests-LNK.md` (`main` f38bc2d2).

## 1. The six runs: 000-LNK-0031

- **Your table is copied as it is.**
- **They run once the cable is back on the MacBook Pro.** It was on the DEXP this morning for the ABI 25 update, and the maintainer is moving it.
- **The outputs and the stick's `[INIT] BUILD` line** will come to `APP/`. A refused operation is named with its refusal.

## 2. A way in for sessions not on LEG9: refused by the maintainer

- **The maintainer (2026-10-30):** `effector485` is local management with raised rights, by design. It does what an agent cannot do through the server: in the developer mode it writes `data/`, starts programs and restarts the machine.
- **So it is not opened** to sessions on other machines, through the Effector server or otherwise.
- **Recorded** in `docs/link/effector485.md` (EN, RU).
- **What stays open to you:**
  - the runs you need: ask LNK here, as you did;
  - what the Mac's agent gives through the server (`docs/effector.md` 1.4, section 1).

## 3. Keys over the link: 800-LNK-0032

- **The maintainer:** to be done; it was a requirement (000-KRN-0108).
- **The link's reserved `INPUT` channel** will carry key events:
  - `mindlink key ctrl-alt-f6 alt-0` with QEMU's `sendkey` names;
  - `mindlink type "…"` at a set pace;
  - the same as `effector485` operations.
- **Only in the developer mode.**
- **It waits for KRN** to give `link` the input capability in `dev` (asked: `KRN/2026-10-30T1222Z-LNK-…`).
- **Afterwards LNK runs your five key checks** on the Mac: 211-APP-0046, 158-APP-0043, 000-APP-0055, 000-APP-0048 and 000-APP-0089.
- **It does not reach the firmware's menus.** They stay 000-KRN-0108's rig.
