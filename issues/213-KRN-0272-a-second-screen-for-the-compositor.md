# 213-KRN-0272 — A second screen for the compositor: `display.wit` 1.3, offered by a display driver and mirrored into

**Type:** `compositor`, IDL (`display.wit` 1.3, `mmio` handles), `init` · **Owner:** `KRN` (the kernel session), for DRV's 213-DRV-0092 · **Priority:** P1 · **Status:** in progress (made, 2026-11-22) · **Blocked by:** — · **Main task:** [213](213-amd-ryzen-pc.md) · **Constitution:** MC-3.11, MC-6.1, MC-12.1, MC-12.3, MC-12.4

## Problem

`display` lit the Ryzen's second output (HDMI-A-1) on 2026-11-22 with the engine's test pattern, through [213-KRN-0271](../issues-done/213-KRN-0271-displays-grant-for-first-light.done)'s grant. DRV's next stage (213-DRV-0092) puts a picture of its own in the range of BAR 0 init granted it, and unblanks the second pipe on it. After that the pixels of a second screen exist, and nothing in the system knows of them.

DRV asked for an interface on 2026-11-22 (`KRN/2026-11-22T0459Z-DRV-request-a-second-screen-offered-to-the-compositor-by-display.md` on `internal-dev-messages`). Through it a display driver tells the compositor:
- that a second screen exists, and when it comes or goes;
- its shape;
- where its pixels are, as a mapping the driver lends;
- and, if wanted, when a frame may be shown.

A mirror of the first screen comes first; an extended screen comes later, with `wm`.

## Plan

1. **`display.wit` 1.3**, served by the compositor:
   - `offer-screen(width, height, pitch, format, pixels: borrow<mmio>)`: the shape (pitch in bytes; `bgr` or `rgb`) and the pixels, a copy of the driver's MMIO range: a lease, as `borrow<…>` is. Offering again replaces the screen before.
   - `screen-gone()`: the compositor lets the mapping go before it answers, so the driver may then revoke its range.
   - Only a client badged `mind::display::BADGE_SCREEN` (3) may call either; others get `denied`, and a shape the range does not hold gets `bad-screen`.
   - No frame timing yet: the compositor writes at its own pace (15 ms), and the second screen may tear.
2. **The IDL tool** takes `mmio` as a handle kind (`CAP_KIND_MMIO`).
3. **`init`** gives `display` its client of the compositor, badged `BADGE_SCREEN`, in `SLOT_DISPLAY`, when the compositor runs.
4. **The compositor mirrors the first screen into the second:**
   - The largest picture of the first screen's shape the second holds, centred, and black around it (`mind::display::fit`). On the Ryzen, 2560x1600 on 1920x1080 is 1728x1080 from x 96.
   - Each picture pixel shows the nearest pixel of the shadow copy.
   - After the first full drawing, only the picture pixels that show a changed span are written (`shown`).
   - The marks the first screen shows (capture, camera, remote) are drawn on the picture too, so no program can hide them there either.
   - On x86 the range maps write-combining, as 213-KRN-0271 maps an MMIO range in the boot framebuffer's BAR.
5. **Not in this task:**
   - frame timing (vertical blank) from the driver;
   - an extended screen, whose layout `wm` needs (APP's to ask for);
   - a second screen on any other machine.

## Acceptance criteria

- [x] `tests/compositor_mirror_host.rs`: the Ryzen's two shapes and others fit centred; each picture pixel shows the nearest source pixel in bounds; a changed source span touches exactly the picture pixels that show it.
- [x] Builds for x86 and aarch64; the IDL fuzzer with `display` 1.3; every gate group passes on both architectures (the compositor runs in all of them, with no second screen).
- [ ] **On the Ryzen** (DRV, with 213-DRV-0092's `display`): the compositor logs `SECOND SCREEN 1920X1080 …: THE FIRST MIRRORED AT 1728X1080 FROM 96,0`, and the maintainer sees MIND Core's screen on both monitors, the marks on both.
- [x] docs/idl and docs/api say what changed.

**Not covered:** whether the second screen tears, and how much; the frame timing is the next step if it does.

## Progress (2026-11-22)

**Made** on `claude/213-KRN-0272-a-second-screen-for-the-compositor`. It is in `fast-test` (d410b274a4) for DRV's run on the Ryzen.

**Runs in this session's container** (QEMU 8.2.2, OVMF):
- `tests/compositor_mirror_host.rs`: 3 of 3;
- the IDL fuzzer with `display` 1.3: 3 of 3;
- the x86 build, and the aarch64 build of `compositor` and `init`;
- x86's `boot`, `display`, `services`, `shell` and `windows` suites on 4 CPUs: 49 PASS, no failure, 3 SKIP for stand-ins not built. No second screen is offered there, and the compositor's screen is unchanged on VGA std, virtio-vga and ramfb.

**Left:**
- the gate's groups;
- DRV's run on the Ryzen;
- the docs criterion, met by docs/idl and docs/api on the branch.

## Landed (2026-11-22)

In `main` since round 169 (8e76876351, `LANDING/2026-11-22T0748Z-KRN-landed-8e768763.md` on `internal-dev-messages`), with DRV's 213-DRV-0086, 0087, 0090, 0091, 158-DRV-0082 to 0084 and APP's request. The gate passed all 61 groups on both architectures, among them the host tests (the mirror's tests and the IDL fuzzer with `display` 1.3). No second screen is offered in any of them.

**Left:** DRV's run on the Ryzen, with 213-DRV-0092's `display` offering HDMI-A-1. A stick written from `main` carries the compositor's part.

## On the Ryzen (2026-11-22)

DRV ran it on the Ryzen with both monitors (`KRN/2026-11-22T0749Z-DRV-your-display-wit-1-3-ran-on-the-ryzen-the-first-screen-mirrored-on-the-second.md` on `internal-dev-messages`):
- **The build:** release 202611220741, boot 76 at 07:45 UTC. It was round 167's candidate with DRV's branches, this branch at ab838d73b, and `display` with all sixteen stages allowed. The tree is not `main`'s, but this task's code is the same.
- **The lines:**
  - 0.517 s: `[COMPOSITOR] SCREEN 2560X1600, STRIDE 2560, FORMAT BGR`;
  - 126.382 s: `[DISPLAY] S15 NO UNDERFLOW OR REQUEST TIMEOUT ON EITHER PIPE IN 300 FRAMES` (the second pipe scanning `display`'s own picture in slot 5's range);
  - 130.596 s: `[COMPOSITOR] SECOND SCREEN 1920X1080, PITCH 7680, Bgr, FROM PID 38: THE FIRST MIRRORED AT 1728X1080 FROM 96,0`;
  - 130.617 s: `[DISPLAY] S16 OFFER DONE; THE FIRMWARE'S PIPE UNCHANGED (30 REGISTERS)`.
- **What they show:**
  - `init` gave `display` the compositor's client badged `BADGE_SCREEN`;
  - `offer-screen` took slot 5's range as a lent `mmio` capability;
  - the compositor mapped it and answered.
- **The trial** was confirmed 14 s after the restart.

This is evidence for that machine, those monitors and that release (MC-12.1).

**Not yet known:**
- what a person sees: whether the mirror follows the first screen, whether the marks show on both, and whether it tears. The maintainer is asked, and DRV sends his words;
- `screen-gone` on hardware: no unplug and no undo were made after stage 16.

## Related

[213-KRN-0271](../issues-done/213-KRN-0271-displays-grant-for-first-light.done), [213-DRV-0086](213-DRV-0086-display-the-second-output-lit.md), [000-KRN-0081](000-KRN-0081-the-framebuffer-written-combined.md) (write-combining), [213](213-amd-ryzen-pc.md).
