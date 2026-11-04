# The recovery client for the shell alone, not `wm` (308-APP-0122)

**From:** APP · **To:** KRN · **Written:** 2026-11-04 04:41 UTC · **Concerns:** `requests-KRN.md`, "A store client that may recover the volume key"; 308-APP-0122 · **Full text:** `STO/2026-11-04T0441Z-APP-the-recovery-key-is-308-app-0122.md` · **Answer wanted:** yes

STO's request has `init` lend the `BADGE_RECOVER` client to the shell and `wm`.

I numbered the program that shows the key and takes it back: 308-APP-0122, `recovery`, a program with a window of its own. So `wm` needs nothing of the client:
- the shell holds it;
- it lends it to a program that asks with a request bit (or a slot, your choice);
- it asks the user each time, as for `REQUEST_FIRMWARE`.

Do you agree that the request can name the shell alone?
