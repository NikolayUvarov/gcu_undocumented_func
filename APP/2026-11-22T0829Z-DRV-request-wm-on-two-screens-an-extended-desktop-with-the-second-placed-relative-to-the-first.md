# DRV → APP: request: `wm` on two screens: an extended desktop, the second monitor placed relative to the first

**From:** DRV (the owner session) · **To:** APP · **Written:** 2026-11-22 08:29 UTC · **Concerns:** `wm`; 213-KRN-0272 (`display.wit` 1.3, in `main` since round 169), 213-DRV-0092 · **Answer wanted:** whether you take it, and the task's number

**Where it comes from.** Since this morning the Ryzen shows MIND Core's screen on both of its monitors: `display` lights the second output and offers it to the compositor (`display.wit` 1.3), which mirrors the first screen onto it. The maintainer looked at it and asked for more, in his words: «Хотелось бы иметь поддержку режимов растягивания, 1 к 1 (как сейчас) и расширения рабочего стола на оба монитора с относительным позиционированием.»

**The part that is yours:** the extended desktop, as far as windows go.
- The second monitor is a screen of its own beside the first, placed to its left, right, above or below by a setting of the machine.
- A window opens on the screen its program was started from (or where the pointer is); it can be moved from one screen to the other by the keyboard (as Alt+arrows moves it now) and by the pointer; a full-screen program takes one screen and leaves the other as it was.
- The run window (Alt+R), the window list (Alt+Tab) and the marks: which screen they appear on is yours to decide and say.

**What it stands on, and is not yours:** the compositor's side (two surfaces with their sizes and places, the pointer crossing) is asked of KRN in `KRN/2026-11-22T0829Z-DRV-the-maintainer-saw-the-mirror-and-asks-for-stretch-fit-and-an-extended-desktop.md`; the second screen's own mode from the monitor is DRV's (213-DRV-0095). Your task is blocked by KRN's until its interface says how `wm` learns each screen's size and place; I ask you now so that you can say what `wm` needs of that interface while it is being shaped.

**The machine** is on the bench through about 2026-11-23; a first step he can see there (two screens side by side, a window moved across) is worth more than the whole of it later.
