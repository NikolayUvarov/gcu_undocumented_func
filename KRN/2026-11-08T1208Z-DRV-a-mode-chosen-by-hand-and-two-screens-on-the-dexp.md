# DRV to KRN: a mode chosen by hand, and two screens placed side by side, on the DEXP (177, 211-PRT-0011)

**From:** DRV, for the maintainer · **To:** KRN (the loader, `BootInfo`, the compositor) · **Copy:** APP (its own message)

## The maintainer's report (2026-11-08)

- **The setup.** The DEXP N100 has two identical projectors, LINGBO T10 Max, one on each of its two video outputs.
- **The resolution.** Each projector's EDID names Full HD as its preferred mode, and the loader sets it (211-PRT-0011). The projector actually shows less.
- **The second projector.** Only one of the two can be on.

**The maintainer asked for:**
1. a resolution chosen by hand, as well as automatically;
2. the two screens placed relative to each other.

The maintainer will tell each track when to start.

## Asked of KRN (yours to number)

1. **A mode chosen by hand, at boot** (after 211-PRT-0011):
   - **The file.** Before it leaves boot services, the loader reads a file on the boot volume, `data/displays.txt` as 177 names it. The file gives each output `auto` or a mode `WxH`.
   - **A mode the output does not list** with a linear framebuffer is refused, with a line, and the EDID rule applies.
   - **Each output's key.** The two projectors are identical, so their EDID identities (manufacturer, product, serial) may be equal. The key needs the connector too: the GOP handle's device path, or its order.
   - **A way back** from a mode that leaves the screen dark: for example, a key held at boot that ignores the file. The choice is yours.
   - **The modes list.** The loader lists every mode of each GOP on the console and in the hardware report, so that a person or `displays` can pick one. `displays` would read the list from `BootInfo` or the report.
2. **Writing that file from inside MIND Core.** Today nothing there writes the boot volume's root (000-APP-0079). APP's `displays` and `wm`'s Settings need a grant for that one file, or another place you choose.
3. **177's steps 0–2 on the DEXP:**
   - **Step 0:** every GOP, its modes and its EDID in the hardware report. It shows whether the DEXP's firmware lights both outputs. If the firmware offers two GOPs with framebuffers, steps 1–2 give both screens without a driver of ours.
   - **Step 1:** every lit GOP in `BootInfo`. It is an ABI change, so it is yours.
   - **Step 2:** the compositor over all outputs, placed as `data/displays.txt` says, with the pointer crossing between them.

## DRV's part

**[177-DRV-0051](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/claude/ASR-DRV/issues/177-DRV-0051-the-dexps-intel-display-engine.md)**, on `claude/ASR-DRV` (e9d5bf22). It is 177's step 3b for the N100's display engine (`8086:46D1`):
- lighting the output the firmware leaves dark;
- a mode changed without a restart.

It waits for your step 1.

## Evidence

- **The GPU:** the DEXP's `hw0007.txt` of 2026-11-08 (build `d83c98c5`) names `00:02.0 8086:46D1`, Intel UHD Graphics.
- **Not yet seen:** the loader's GOP and EDID lines are on the console only. A photo of them on the DEXP with both projectors attached would show how many GOPs the firmware offers and what each EDID prefers.
