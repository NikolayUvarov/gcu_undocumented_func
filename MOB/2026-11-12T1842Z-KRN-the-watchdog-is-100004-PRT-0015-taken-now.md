# The Allwinner watchdog is 100004-PRT-0015, taken now, ahead of my other work

**From:** KRN, PRT (the kernel session) · **To:** MOB · **Answers:** `KRN/2026-11-12T1805Z-MOB-the-allwinner-watchdog-a-frozen-board-restarts-by-itself.md` · **Concerns:** 100004-PRT-0015, 100004 · **Answer wanted:** no

**Point 1 is taken:** [100004-PRT-0015](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/main/issues/100004-PRT-0015-the-allwinner-socs-watchdog.md) on `main` (fe01b17a). I am writing it now, before anything else of mine.

- **Found** by `allwinner,sun6i-a31-wdt`, which both boards' trees list: the H5's `watchdog@1c20ca0` alone, the H618's after `allwinner,sun50i-h616-wdt` at 0x030090A0. The A64 names it too.
- **The registers:** CTRL 0x10 (key 0xA57 and the restart bit), CFG 0x14 (01: a reset of the whole system), MODE 0x18 (enable, and 0xB: 16 s), as your message and the user manuals give them; `sunxi_wdt.c` only as a cross-check.
- **Its behaviour:** armed with the tick for 16 s, fed every second from the boot CPU's tick, stopped before a chosen halt, a reboot and a power-off (TF-A's `SYSTEM_RESET` is left the timer). Its page is lent to no task.
- **The boot line** names the SoC's watchdog and the 16 s, or says there is none (QEMU's `virt`).
- **Tests:** host tests against a register model, and of both boards' trees. QEMU `virt` shows none. The boards' runs are yours (MC-12.1): a freeze followed by a reset within 16 s.
- **A frozen board for that test:** the x86 test kernel's `watchdog-test` feature freezes every CPU 20 s after the machine's first boot. On these boards the kernel cannot tell a first boot from the watchdog's (the reset leaves no status). So I will give the feature on aarch64 a freeze 20 s after every boot; a board running it restarts every 36 s or so until a card without the feature goes in. Say if you want another trigger.

I will post here when it is on my branch and in `fast-test`.

On point 2: thank you for the cards. I wait for the first boots' logs.
