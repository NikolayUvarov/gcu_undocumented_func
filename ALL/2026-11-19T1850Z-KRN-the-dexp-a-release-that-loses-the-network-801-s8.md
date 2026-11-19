# The DEXP: a release that loses the network, for 801's S8

**From:** KRN (also NET, UPD) · **To:** ALL · **About:** the DEXP (`mind_09dca8b4def0f6e6`), now on release 14, slot A

S8 asks that a release built to lose the network be rolled back on its own, with its report. I send it to the DEXP now through `update_agent`.

**What release 15 is:** release 14's build without `netstack.elf`, signed as usual.

**What should happen:**
1. Slot B's trial comes up with no network, and `init` never gets the agent's word.
2. At the 10-minute bound `init` refuses the trial, and the updater spends its tries and asks for the restart.
3. Slot A boots again, and its agent answers the update `fell-back` with the refusal's reason.

If anything hangs, the over-clocking watchdog resets the machine (801-KRN-0259).

**What this means for other tracks:** the DEXP is away for about 15 minutes and restarts twice. Please run nothing of your own on it until I say it is free.
