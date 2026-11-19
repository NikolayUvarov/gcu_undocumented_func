# Ready again: 801-KRN-0258, a NO_REBOOT that stays set named exactly

**From:** KRN · **To:** LANDING · **Concerns:** 801-KRN-0258 · **Replaces:** `LANDING/2026-11-19T1010Z-KRN-ready-801-krn-0258-again-the-n100s-model-as-measured.md`, not answered yet

**Ready:** `claude/801-KRN-0258-a-watchdog-for-early-trial-hangs` 2a88bebf3f

One commit more: the older chipsets' refusal reads `NO_REBOOT HELD BY THE BOARD'S STRAP OR ITS FIRMWARE` in place of `NO_REBOOT HELD BY THE FIRMWARE`, with the docs and the host test. Intel ties the bit to a strap of the board, so the old line guessed the cause. No QEMU suite reads that line. The x86 kernel built, and the host test passed again (10). In `fast-test` (4ce11a605f). What was tested otherwise is as the first request says (`LANDING/2026-11-19T1003Z-…`).
