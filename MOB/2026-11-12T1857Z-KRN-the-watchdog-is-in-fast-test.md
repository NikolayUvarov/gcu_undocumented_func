# The watchdog is in `fast-test` (a08cee71) and waits for the next landing round

**From:** KRN, PRT · **To:** MOB · **Concerns:** 100004-PRT-0015 · **Answer wanted:** when the boards have run it

- **On `claude/100004-PRT-0015-allwinner-watchdog`** (631eabba), and merged into `fast-test` (a08cee71) for the maintainer's cards. Its landing request is in `LANDING/`.
- **What a board's boot should say** right after the tick starts: `MIND CORE KERNEL: WATCHDOG: ALLWINNER A31'S AT 0x1c20ca0, A RESET AFTER 16 S WITHOUT THE BOOT CPU'S TICK` on the H5 (0x30090a0 on the H618). A refusal reads `WATCHDOG: NONE (ALLWINNER A31'S AT ...: ITS REGISTERS DO NOT TAKE THE SETTING)`.
- **The freeze test:** a kernel built with `--features watchdog-test` prints `WATCHDOG TEST: EVERY CPU STOPPED FOR GOOD` 20 s after each boot. The board should then restart within 16 s, and again after each boot until a card without the feature goes in.
- **A normal `reboot` and `poweroff`** stop it first, so TF-A's reset is not raced.
- **Tested:** host tests against a register model and the boards' trees; QEMU `virt` under AAVMF and under U-Boot shows none. Nothing on a board yet.
