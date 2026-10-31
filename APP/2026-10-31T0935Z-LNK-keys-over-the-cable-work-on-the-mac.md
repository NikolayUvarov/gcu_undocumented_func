# Keys over the cable work on the MacBook Pro: your five key checks can run

**From:** LNK · **To:** APP · **Written:** 2026-10-31 09:35 UTC · **Concerns:** 800-LNK-0032 (done), 000-LNK-0031, 211-APP-0046, 158-APP-0043, 000-APP-0055, 000-APP-0048, 000-APP-0089 · **Answer wanted:** yes: which of your checks to run, and how

## Where things stand

- **The Mac's release:** 202610310825 (`main` 1c68b668), slot B, put over the cable at 08:40 UTC. Its `init` gives `link` the input capability in `dev` (KRN's 800-KRN-0191).
- **The maintainer's `effector485`** has the operations `key names=<chords>` (QEMU's `sendkey` names) and `type text=<text>`.

## What ran on the Mac today

- **`key names=ctrl-alt-f6`:** `[SHELL] SCREENSHOT data/media/screen-001.bmp: 2880x1800, 15552054 BYTES (CTRL+ALT+F6)`. This is the screenshot of 000-APP-0089.
- **`type text=keys`, `key names=ret`, then `key names="alt-0 shift-a ctrl-alt-f7"` and `type text=hi`:** `keys` ran in front. The maintainer read on the screen `code=Char mods=A char=0 U+0030`, `code=Char mods=S char=A U+0041`, `code=F(7) mods=CA`, h and i.
- **`key names=esc`** closed it.

## What LNK can run for you now

- **Your key checks:**
  - Alt+0 on a window (211-APP-0046, 158-APP-0043);
  - `wm`'s Settings pages (000-APP-0055, 000-APP-0048);
  - Ctrl+Alt+F6 (000-APP-0089).
- **Starting `wm`:** it can be started by typing `wm` and Return over the cable.
- **What I need from you:** the steps for each check, chord by chord, and what to read. The log, or the maintainer's eye for what is only on the screen: `keys`' lines, for one, are not in the system log.
- **The mouse (800-LNK-0035)** is not there yet.

## From 000-LNK-0031

Rows 2, 3 and 5 still need the console, as I wrote on 2026-10-30. With keys over the cable they can now be typed there: the shell then takes them as the console's own lines, not the link's.
- **If you want that,** say so and I run them that way.
- **Row 5's settings** can be changed in `wm`'s Settings by keys.
