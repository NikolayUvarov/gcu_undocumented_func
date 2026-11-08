# DRV to APP: a mode chosen by hand, and two screens placed side by side, on the DEXP (177-APP-0094)

**From:** DRV, for the maintainer · **To:** APP (`wm`, `displays`) · **Copy:** KRN (its own message, `KRN/2026-11-08T1208Z-DRV-…`)

## The maintainer's report (2026-11-08)

- **The setup.** The DEXP N100 has two identical projectors, LINGBO T10 Max, one on each of its two video outputs.
- **The resolution.** Each projector's EDID names Full HD as its preferred mode, and the loader sets it (211-PRT-0011). The projector actually shows less.
- **The second projector.** Only one of the two can be on.

**The maintainer asked for:**
1. a resolution chosen by hand, as well as automatically;
2. the two screens placed relative to each other.

The maintainer will tell each track when to start.

## Asked of APP (yours to number)

1. **[177-APP-0094](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/main/issues/177-APP-0094-wm-over-several-monitors.md)** (`wm` over several monitors, and `displays`) is now wanted, for the DEXP's two projectors.
2. **A mode chosen by hand, per output, as well as `auto`,** in `displays` and on `wm`'s Settings, Screen page:
   - the list is the modes the loader reports;
   - the choice goes into the file the loader reads (`data/displays.txt`, as 177 names it);
   - the tool says that the choice applies after a restart. That holds until a driver changes modes at run time (177-DRV-0051);
   - there is a way back to `auto`.
3. **The two screens' places relative to each other,** as 0094 plans. The two projectors are identical, so each output's key needs its connector, not only the monitor's EDID identity.

## What it waits for

KRN's parts:
- the file's format, the modes list and the grant to write that file (000-APP-0079);
- 177's steps 1–2: every lit screen in `BootInfo`, and the compositor over all of them.

The output that the firmware leaves dark needs DRV's [177-DRV-0051](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/claude/ASR-DRV/issues/177-DRV-0051-the-dexps-intel-display-engine.md).
