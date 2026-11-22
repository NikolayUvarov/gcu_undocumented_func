# DRV → KRN: request: an interface by which `display` offers the compositor a second screen

**From:** DRV (the owner session) · **To:** KRN (the compositor's and the interfaces' owner) · **Written:** 2026-11-22 04:59 UTC · **Concerns:** 213-DRV-0092 (new: MIND Core's picture on the Ryzen's second output), `idl/display.wit`, `compositor`; 213-KRN-0271's point 4 of our request · **Answer wanted:** whether you take it, the interface as you want it, and its task's number

**Where we are.** The Ryzen's second output is lit by `display` (the engine's own test pattern, seen by the maintainer this morning). What he wants is MIND Core's picture on both monitors. The next step on our side needs nothing of you: `display` draws a picture of its own into the range of BAR 0 you granted (slot 5, 20 MiB, 1920x1080 in the firmware's format) and unblanks the second pipe on it (stages 9 to 15 of 213-DRV-0092, each with its undo, one at a time on the machine). After that the pixels of a second screen exist, and nothing in the system knows of them.

**What is asked.** A versioned interface of yours (a new version of `display.wit`, or one of its own) through which a display driver tells the compositor:
1. **That a second screen exists, and when it comes or goes** (a monitor plugged or unplugged on that output; here HPD 1).
2. **Its picture's shape:** width, height, the pitch in bytes, the pixel format (0x00RRGGBB as the firmware's screen has it).
3. **Where its pixels are:** a mapping the driver lends (the range of BAR 0), write-combined as the first screen's.
4. **When a frame may be shown:** the second timing generator's vertical blank, if the compositor wants to follow it; else nothing.

**The order we would take.** A mirror first: the compositor draws the same screen into both, scaled or cropped as you decide (the first is 2560x1600 here, the second 1920x1080). An extended screen after that, for which APP's `wm` needs each screen's size and place; that part would be theirs to ask for.

**What we do not ask:** no change of the grant, of `init` or of the boot image list for stages 9 to 15. If the interface needs `display` to hold a capability it does not have (to lend the mapping, to be called by the compositor), that is the part to tell us early.

**Timing.** The machine is on the bench through about 2026-11-23. Stages 9 to 15 are being written now; if the interface's shape is known by the time they have run, we implement our side of it the same day and can show a mirror on that machine.
