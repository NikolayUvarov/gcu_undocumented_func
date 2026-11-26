# 213-DRV-0112 — `display`: a monitor on the Ryzen's other HDMI socket (HDMI-A-2)

**Type:** driver (`display`) · **Owner:** `DRV` · **Priority:** P2 · **Status:** open (2026-11-26) · **Blocked by:** — · **Main task:** [213](213-amd-ryzen-pc.md) · **Roadmap:** the drivers track · **Constitution:** MC-12.1, MC-12.2, MC-12.3, MC-12.5

## Problem

The Ryzen has two HDMI sockets and a USB-C one that carries the first display. `display` drives only one of the HDMI
sockets: HDMI-A-1. A monitor plugged into the other gets no picture.

**The maintainer's report, 2026-11-26, boot 107:** «переключил несколько раз и попробовал перейти на hdmi1, он не
запустился», and then, asked which socket that was: «да, на Ryzen два HDMI порта и один type-c с дисплеем».

**What the log shows of it.** At 2050.628 s `STEP 3 TAKEN BACK: HPD 1 SENSES NO MONITOR`, and then no line of `display`
for 19 s, until the cable was back in the first socket (2069.618 s, lit again at 2071.802 s). `display` follows HPD 1
alone, so it never saw the monitor on the other socket.

**Seen in the same seconds, not explained:** at 2054.3 s the hub on xHCI 1's port 1 (05E3:0608, with the mouse's
receiver and the serial converter behind it) went away and came back. Whether the hub is in that monitor is asked of
the maintainer.

## What is known

From the survey ([213-DRV-0085](../issues-done/213-DRV-0085-the-radeons-display-engine-surveyed.done),
`hwdocs/cezanne-display.md`, "The display paths"):

| | HDMI-A-1, driven today | HDMI-A-2, this task |
|---|---|---|
| VBIOS path | 0 | 1 |
| Connector | `330C`, HDMI type A, enum 3 | `340C`, HDMI type A, enum 4 |
| Encoder, PHY | `211E`, UNIPHY enum 1: PHY A | `221E`, UNIPHY enum 2: PHY B |
| DDC | line 0, `DC_GPIO_DDC1` | line 1, `DC_GPIO_DDC2` |
| HPD | pin 1: `DC_GPIO_HPD_A` bit 0, HPD 1 | pin 2: bit 8, HPD 2 |

- Linux names them in that order; which printed label each socket carries is not recorded. The maintainer calls the
  undriven one "hdmi1".
- Nothing of PHY B, of DIG back end B, of DDC2 or of HPD 2 has been read on this machine by MIND Core. What the
  firmware leaves there is not known.
- There is one spare pipe: the firmware's picture takes one, the second screen the other. So one HDMI socket at a time.

## Plan

1. **Read first.** Step 1's batches read, and never write, what steps 2 to 4 need of the second socket: HPD 2's sense
   and control, DDC2's lines and the I2C engine's DDC2 registers, PHY B and DIG back end B as the firmware left them.
   A run on the Ryzen with the monitor in the second socket records the values here before any write is designed on
   them.
2. **The output as a parameter.** `HDMI_A_1` is fixed in the choice of the pipe, in the EDID read (`ddc.rs`), in the
   follower of the plug (`start.rs`), in the guard of what may be written, and in the tables' parameters. Each takes
   the output, with host cases for both.
3. **Which socket.** `display` follows both senses and lights the socket that has a monitor. With a monitor in each,
   the first lit stays; the other is named in the log as not driven, with the reason (one spare pipe).
4. **The guard.** The registers of the second socket's PHY, back end, DDC and HPD join what may be written only for
   that output; the firmware's pipe and the other socket's blocks stay refused. A host case for each refusal.
5. **On the Ryzen, in stages as HDMI-A-1 was:** the read-only run; the EDID read; first light with the test pattern;
   MIND Core's picture; the plug taken out and put back; the monitor moved from one socket to the other.

## Acceptance criteria

- [ ] Step 1 reads the second socket's blocks, with host cases that it writes nothing; its values from the Ryzen
  recorded here.
- [ ] The EDID read over DDC2, with host cases on the model of the engine; on the Ryzen its checksums good and the
  engine left as found.
- [ ] Host cases: the stages on a model with the monitor on the second socket; the guard's refusals for the first
  socket's blocks then; a monitor in each socket.
- [ ] On the Ryzen: a monitor in the second socket lit in its own mode with the mirror on it, stage 15's watch clean,
  the firmware's pipe unchanged, and the maintainer's look (MC-12.1).
- [ ] On the Ryzen: the monitor moved from one socket to the other and back, lit each time.
- [ ] HDMI-A-1 alone behaves as before: its host cases pass unchanged and its lines on the Ryzen are the same.

## Related

[213-DRV-0095](../issues-done/213-DRV-0095-display-the-second-outputs-mode-from-its-edid.done) (the mode from the EDID),
[213-DRV-0096](../issues-done/213-DRV-0096-display-the-second-output-plugged-and-unplugged.done) (the plug followed),
[213-DRV-0092](213-DRV-0092-display-mind-cores-picture-on-the-second-output.md) (the picture and the mirror),
[213-DRV-0078](../issues-done/213-DRV-0078-a-display-driver-for-the-ryzens-radeon.done) (the driver's main record).
