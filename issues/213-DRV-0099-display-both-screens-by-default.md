# 213-DRV-0099 — `display`: both screens by default

**Type:** driver (`display`) · **Owner:** `DRV` · **Priority:** P2 · **Status:** open (2026-11-22) · **Blocked by:** [213-DRV-0098](../issues-done/213-DRV-0098-display-both-screens-at-start-in-seconds.done); the maintainer's decision · **Main task:** [213](213-amd-ryzen-pc.md) · **Roadmap:** the drivers track · **Constitution:** MC-12.1, MC-12.3, MC-12.5

## Problem

The committed switches are all off: `BATCHES_ALLOWED` 0, `STAGES_ALLOWED` 0, `UNDO_AFTER_STAGES` false. Each run on the
Ryzen was a release built with other values on the owner session's word. A machine meant to simply come up with both
screens needs a default that does it, and the evidence that the default is safe on the machines it reaches.

## Plan

- **Where the decision lives:** the switches leave the build and become a per-machine setting, kept with the machine's
  other settings, so that a release is the same for every machine.
- **The default on** only where the evidence covers the machine: the Ryzen with its two monitors (MC-12.1). Elsewhere,
  `display` reads and logs, and writes nothing.
- **What happens on a failure** at start: the undo, and the machine comes up with its first screen alone, said once in
  the log.
- **The maintainer decides** where the default is on.

## A proposal (2026-11-22), for the maintainer's decision; nothing of it is built

**Where the setting lives: one line in the machine's own `data/displays.txt`**, for example `output second light`.
- **Whose file it is:** the screens' file. The shell (213-APP-0158) keeps the mirror's fit there (`second keep|stretch`),
  and 177 plans the screens' layout in it. Its reader leaves a line it does not know for that layout.
- **What keeps it with its machine:** it is on the machine's own disk. `build_and_write.sh --keep-settings` keeps all of
  `data/` for each machine across a rewrite, in `~/.config/mind/machines/<id>/settings/` (800-LNK-0024, 801-LNK-0046),
  so the choice stays with the machine it was made for and reaches a new disk too.
- **Places not taken:**
  - **The owner's policy** (`data/effector/`) is the Effector agent's: what may be done from afar, not how a device starts.
  - **A build switch** would keep a release different for each machine, which is what this task removes.
  - **A file of its own** (`data/display.conf`) would do as well, but it would make two files for the screens.

**Who reads it: `init`, at `display`'s start.** It reads it as it reads nnd's line of `data/memory.conf` (262-KRN-0178)
and passes it in `display`'s argument after the device's index (`24 light`). So `display` needs no file client and no
new grant. With no line, or a value it does not know, the setting is `off`, and `init` says which.

**What it holds: one of three values, for which the six switches collapse.**

| Value | What `display` does | Today's switches |
|---|---|---|
| `off`, also with no line | maps no register, reads and writes nothing | `BATCHES_ALLOWED` 0, `STAGES_ALLOWED` 0 |
| `read` | step 1, read only, and the stages described; nothing written. Step 2 is left out, since it writes the I2C engine | 11, 0; `EDID_ALLOWED` false |
| `light` | steps 1 to 4 in the production order (213-DRV-0098): the EDID, the monitor's own mode, the 16 stages and, once 213-DRV-0096 has run on the hardware, HPD 1 followed | 11, 16; `EDID_ALLOWED`, `HDMI_SIGNALLING`, `MODE_FROM_EDID` and `HOTPLUG` true |

- **What becomes of the switches:**
  - `HDMI_SIGNALLING` and `MODE_FROM_EDID` go: under `light` the EDID decides. An HDMI sink gets its preferred timing in
    HDMI when it fits (`light::mode_from_edid`), and anything else 1920x1080, in HDMI for an HDMI sink and in DVI otherwise.
  - Three stay build switches for bring-up only, off in a release: `SURVEY_ORDER`, `UNDO_AFTER_STAGES` and a
    `STAGES_ALLOWED` below 16. A build with one of them set is a bring-up build and says so in its first line.

**What a failure at start does with `light`: what it does now, on the machine's own disk.**
- **A stage that fails:** it is undone with every stage before it, so the second pipe is off, and the firmware's pipe is
  read back and said unchanged. The machine comes up with its first screen alone.
  - One line says so: `STEP 3 FAILED AT STAGE n ... THE FIRMWARE'S PIPE UNCHANGED`, with the failing stage's detail
    before it (the production order).
  - The mark stays, so nothing is tried again until the next boot: no loop, no retry on a plug.
- **Stage 9, 10 or 16 not ready:** first light, or the picture, stays as it is, and the shell's `screen second` says that
  no display driver offers a second screen.
- **`display` faulting:** `init` restarts it, the mark means no stage runs again, and DP-2 was never written.
- **Not proposed now:** telling the shell why there is no second screen. That would need `display.wit` to carry a state
  (KRN).

**Where `light` is set: nowhere by default.**
- A machine with no line is `off`.
- The maintainer writes `light` into the Ryzen's `data/displays.txt`. It is the one machine the evidence covers (MC-12.1):
  boots 70 to 81 in the survey's order and the production series from boot 84, on that machine with its two monitors.
- `init` starts `display` only where it finds the Radeon 1002:1638, so no other machine reads the line at all.
- Whatever the line says, `display` still refuses what it cannot tell: `assess`'s checks, the DMCUB running, no VBIOS
  image, the mark.

**What each track would provide:**
- **KRN (`init`):**
  - read `display`'s line at its start and pass it in the argument, as for nnd's memory;
  - a parser in libmind (as `mind::memconf` is), so that `init` and the shell read the file alike.
- **APP (the shell):**
  - `screen second light|read|off` writes the line, and `screen second` shows it.
  - **Found on the way:** the shell writes the whole file from the fit alone. `screenctl.rs` calls
    `File::create(screens::FILE)` and then `write_at(0, screens::text(fit))`. So a line added by hand, or by this
    setting, would be lost at the next `screen second keep|stretch`. The writer would have to keep the lines it does
    not know, as its reader does.
- **LNK:** nothing new is needed. `--keep-settings` keeps `data/displays.txt`; a first disk for the Ryzen gets the line
  from the shell, or from a `--display light` if the maintainer wants one.
- **DRV (`display`):**
  - the argument read and mapped to what the switches do now;
  - host cases for `off`, `read`, `light`, an unknown value and none;
  - a failure at each stage with `light` (the undo cases exist; they would run under the setting).

## Acceptance criteria

- [ ] The setting, read at start; host cases for on, off and a failure at each stage with the setting on.
- [ ] On the Ryzen: both screens come up at every start with no build switch changed, over several starts.

## Related

[213-DRV-0092](213-DRV-0092-display-mind-cores-picture-on-the-second-output.md), [213-DRV-0098](../issues-done/213-DRV-0098-display-both-screens-at-start-in-seconds.done).
