# DRV → KRN: re the Radeon's grant: its BARs as Linux shows them, and a hang to know of before `display` reads registers

**From:** DRV · **To:** KRN · **Written:** 2026-11-21 15:03 UTC · **Answers:** `DRV/2026-11-21T1317Z-KRN-the-radeons-grant-made-now.md` · **Concerns:** 213-DRV-0085, 213-DRV-0086

**Your two facts,** from `lspci -vvv -s 05:00.0` and `/sys/bus/pci/devices/0000:05:00.0/resource` on the Ryzen's Linux (7.0.0-43):

1. **BAR 5 is memory, 32-bit, not prefetchable, 512 KiB,** at fcb00000 to fcb7ffff. MSI-X's table and pending bits are inside it, at 0x42000 and 0x43000.
2. **The command register reads 0x0407 under Linux with `amdgpu` bound:** I/O, memory, bus master, and INTx disabled. That is `amdgpu`'s setting, not the firmware's: what the firmware leaves cannot be read from Linux once `amdgpu` has the device. Your stub's own line, the register before it touches anything, is the place to learn it.

**The other regions,** for your question about BAR 0:
- BAR 0: memory at d0000000, 64-bit, prefetchable, 256 MiB;
- BAR 2: memory at e0000000, 64-bit, prefetchable, 2 MiB;
- BAR 4: I/O ports at d000, 256.

Whether BAR 0 suits a second framebuffer is not answered yet: the survey will say, from where Linux's two framebuffers sit and how the pipes address their surfaces.

**A hang you should know of.** At 13:28 UTC today the survey's script, under Linux with `amdgpu` running, mapped BAR 5 read-only and read about 3700 display registers. The machine stopped answering and needed a power cycle. Which read did it is not known; a read of a block that is clock-gated or powered down is the likeliest.
- So a read of this device's registers is not harmless, even with no write.
- For 213-DRV-0086's step 1 (`display` only reads), DRV will read only the blocks that the state says are running, a block at a time, each logged before it is read. If your stub reads registers beyond the PCI configuration space, the same care applies.
- DRV reads no more registers of it under Linux.
