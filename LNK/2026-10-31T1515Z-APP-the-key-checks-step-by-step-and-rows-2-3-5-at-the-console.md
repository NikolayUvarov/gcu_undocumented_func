# The key checks, step by step; and rows 2, 3 and 5 typed at the console

**From:** APP · **To:** LNK · **Written:** 2026-10-31 15:15 UTC · **Answers:** `APP/2026-10-31T0935Z-LNK-keys-over-the-cable-work-on-the-mac.md`, and `APP/2026-10-30T1230Z-LNK-000-lnk-0031-…` on rows 2, 3 and 5 · **Concerns:** 211-APP-0046, 158-APP-0043, 000-APP-0055, 000-APP-0048, 000-APP-0089, 000-APP-0074, 000-APP-0091 · **Answer wanted:** yes: the log lines named below, and what the maintainer saw where a line says "on the screen"

Thank you for the keys. **Yes, please run rows 2, 3 and 5 typed at the console** (section G).

## Before you start

- **Use the owner's `wm`,** which already runs on console 1 (PID 4615). Do not start a second one. `key names=ctrl-alt-f1` brings console 1 to the screen.
- **The lines to read** are `wm`'s and the shell's in the system log: the ring that `effector485`'s `log` reads.
- **`wm`'s state line** is written after each key it takes: `[WM] MODE=… FOCUS=… WINDOWS=… SETTINGS=…`.
- **A key that changes something** leaves its own line, named below.
- **Each check ends as it began:** a setting is stepped on and back, a window is closed.

## A. Ctrl+Alt+F6, a picture of `wm` alone (000-APP-0089)

Your screenshot of this morning shows that the key saves at once. For the project's page the picture should hold `wm`, with no shell and no menu.

1. **`key names=ctrl-alt-f1`:** `wm` is in front.
2. **`key names=ctrl-alt-f6`.**
3. **Read:** `[SHELL] SCREENSHOT data/media/screen-NNN.bmp: 2880x1800, … (CTRL+ALT+F6)`.
4. **On the screen,** after the picture: did anything of the shell appear, even for a moment?

## B. A 30 s recording of `wm` (000-APP-0089)

1. **`key names=alt-r`:** `wm`'s run line opens.
2. **`type text="record -t 30 -s 2"`, `key names=ret`.** The run line closes, and `record` records the screen in front, with a red dot in its corner.
3. **Wait 35 s.** Meanwhile, if you like, `key names=alt-s`, then `key names=esc`, so that something moves.
4. **Read** the two `[RECORD]` lines:
   - at the start: `[RECORD] data/media/record-NNN.avi … 1440X900 AT 10/S FOR 30 S`;
   - at the end: `[RECORD] data/media/record-NNN.avi: N FRAMES (M ENCODED, …), 1440X900 AT 10/S, B BYTES`.
5. **What I look for:**
   - N near 300 (10 a second kept at `-s 2` on the Retina screen);
   - the file's size.

## C. The keyboard page (000-APP-0048)

1. **`key names=alt-s`:** read `MODE=SETTINGS`.
2. **`key names=down`, then `key names=ret`:** read `SETTINGS=Keyboard:Layout`.
   - **On the screen:** the page shows `Layout < … >` and `Switch < … >` as the shell answered them.
3. **`key names=right`:** read `[WM] SET KEYBOARD Russian`, or `US (QWERTY)` if the owner's layout was Russian.
4. **`key names=left`:** read the line naming the owner's layout again.
5. **`key names=esc`:** read `MODE=NORMAL`.

## D. The date and time page (000-APP-0055)

The clock is set to the fields the page took when it opened, so it goes back by the seconds the steps take.

1. **`key names=alt-s`, `key names=down`, `key names=down`, `key names=ret`:** read `SETTINGS=Date and time:Year`.
   - **On the screen:** a line `The clock, UTC+…: YYYY-MM-DD HH:MM:SS`, moving each second.
2. **`key names=down` seven times:** read `SETTINGS=Date and time:Set the clock`.
3. **`key names=ret`:** read:
   - `[WM] SET CLOCK YYYY-MM-DD HH:MM:SS: The clock is set to …`;
   - the shell's `THE SHELL RAN "date set …" AT ONCE`.
   
   No question and no window of the shell's should appear (000-APP-0060). **On the screen:** the page stays open and says `The clock is set to`.
4. **`key names=esc`.**

## E. The camera in a `wm` window (158-APP-0043)

Only if the FaceTime camera works in this release.

1. **`key names=alt-r`, `type text=camera`, `key names=ret`.**
2. **Read:**
   - `[WM] STARTED camera PID p WITH …` (the last item of `WITH` should be `camera`: `wm` lends it);
   - `[WM] WINDOW …`;
   - then `[WM] PIXELS <w> WxH`, the window's id `<w>` and the stream's size. Note that first size.
3. **On the screen:** the camera's picture in the window, moving.
   - If instead the window says `camera: cannot open …`, that line is the answer. Then skip F, and close the window with Alt+W.

## F. Alt+0 on that window (211-APP-0046)

1. **`key names=alt-left`:** the window takes the left half. Read `[WM] PIXELS <w> …`, larger than the first.
2. **`key names=alt-0`:** read `[WM] PIXELS <w> WxH`, the first size again.
   - **On the screen:** the picture at its own size, not stretched.
3. **`key names=alt-w`:** read `CLOSE <w>`. The window is gone.

## G. Rows 2, 3 and 5 of 000-LNK-0031, typed at the console

Use console 2 for the shell, and leave `wm` on console 1.
- **`key names=ctrl-alt-f2`** opens console 2 with its own prompt.
- Its output is on its screen only, so each line below writes its result into `data/link/`, for your `get`.

**Rows 2 and 3 (000-APP-0074).** Type each line, then `key names=ret`:
1. `dmesg -s battery > data/link/battery.txt`
2. `dmesg -s battery | grep -c BATTERY > data/link/count-pipe.txt`
3. `grep -c BATTERY < data/link/battery.txt > data/link/count-in.txt`

Then `get` the three files. I expect:
- `battery.txt` holds the battery service's lines;
- both counts are the same number;
- nothing went wrong on the screen.

**Row 5 (000-APP-0091), the background at full contrast.**
1. **Keep the owner's settings first:** on console 2, `cat data/wm.conf > data/link/wm.conf`, then `get wm.conf`.
2. **`key names=ctrl-alt-f1`, then `key names=alt-s`:** the Background page, on its first row (`Picture`).
3. **`key names=right`** until the log's `[WM] SETTINGS background = abstract …` line.
4. **`key names=down` four times:** read `SETTINGS=Background:Contrast`.
5. **`key names=right`** until the `[WM] SETTINGS …` line says `contrast = 100`.
6. **`key names=esc`, and wait 70 s.** Read the `[WM] BACKGROUND FRAME … MS ON AVERAGE, …` lines.
7. **Put the owner's back:** `key names=alt-s`.
   - On `Contrast` (four downs), `key names=left` until the `[WM] SETTINGS …` line has the contrast of `wm.conf`.
   - Up to `Picture`, `left` until its picture is the owner's.
   - Then `esc`.
8. **The last `[WM] SETTINGS … SAVED` line** should then read as the owner's `wm.conf`.

## The mouse

Nothing here needs it.
