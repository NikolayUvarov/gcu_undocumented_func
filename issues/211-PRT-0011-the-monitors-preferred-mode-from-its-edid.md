# 211-PRT-0011 — The bootloader sets the monitor's preferred mode from its EDID

**Type:** bootloader · **Owner:** `KRN` (`PRT`, `bootloader/`) · **Priority:** P1 · **Status:** open (made; a run on the DEXP left) · **Blocked by:** — · **Main task:** [211](211-intel-pc-from-a-sata-ssd.md) · **Roadmap:** track A (real hardware) · **Constitution:** MC-12.1, MC-12.3

**Asked by:** the drivers track (`DRV`) for the maintainer, in `KRN/2026-10-27T1535Z-DRV-the-bootloader-keeps-800x600-on-the-dexp.md` and `KRN/2026-10-27T2155Z-DRV-the-dexps-screen-is-next.md` on `internal-dev-messages`. The maintainer: "the resolution does not match the monitor".

## Problem

The DEXP desktop showed its monitor's own resolution once, and 800×600 on the next boot from another USB port (`[COMPOSITOR] SCREEN 800X600`). The loader kept whatever mode the firmware left whenever that mode had a linear framebuffer, and chose a mode itself only for a BltOnly one.

## What was done

- **`bootloader/src/edid.rs`** (host-tested in `tests/edid_host.rs`):
  - `preferred`: the monitor's preferred mode, the first detailed timing descriptor of the EDID's base block (bytes 54–71), after the header and the block's checksum are checked;
  - `choose`: the mode to use.
- **The rule:**
  1. **The EDID's preferred mode, when the output lists it with a linear framebuffer,** is set (or kept, if current). The EDID comes from the chosen output's handle: `EFI_EDID_ACTIVE_PROTOCOL`, else `EFI_EDID_DISCOVERED_PROTOCOL`.
     - A preferred mode larger than 2880×1800, the largest screen the services are sized for (211-KRN-0018), is not set.
  2. **Otherwise the firmware's mode stays** if it has a linear framebuffer.
  3. **Else** the largest drawable mode up to 1920×1200, as before.
- **The lines that say so,** on the console (and so `data/pause.txt`'s photo pause):
  - `GOP n: EDID (ACTIVE|DISCOVERED, N BYTES): THE MONITOR PREFERS WxH`, or `GOP n: NO EDID`, or why the EDID gave none;
  - `M MODES; SET WxH: THE MONITOR'S PREFERRED`, or `KEPT THE FIRMWARE'S WxH: …` with the reason.

**Not done as asked:** without an EDID, DRV asked for the largest mode up to 1920×1200 instead of the firmware's mode. The loader keeps the firmware's mode there, for two reasons:
- **A dark screen.** A mode the monitor never said it shows may leave the screen dark, with no line to read.
- **The tested configurations.** In QEMU (x86's 1280×800, aarch64's 800×600) the size would change under the suites that read the screen.

If the DEXP's run shows `NO EDID`, this is the place to revisit.

## Acceptance criteria

- **Host tests:** `tests/edid_host.rs` — the preferred timing, the refusals (short, header, checksum, no timing), the choice with and without an EDID, a 4K preferred mode kept out, the Retina size let in.
- **Builds** for x86 and aarch64; the boot suite passes in QEMU on x86 (its GOP lines included).
- **On the DEXP:**
  - the screen comes up at the monitor's preferred mode on every port;
  - the lines show the EDID's source and the mode set.

  The machine and the build are named (MC-12.1).

## Related

[211](211-intel-pc-from-a-sata-ssd.md), [211-KRN-0016](../issues-done/211-KRN-0016-the-screens-gop-and-boot-progress.done) (the chosen GOP), [211-KRN-0018](../issues-done/211-KRN-0018-the-compositors-quota-fits-the-screen.done) (the largest screen).
