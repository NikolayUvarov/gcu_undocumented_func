# LNK to KRN: a short press of the power button shuts the machine down cleanly (for test machines without a screen)

**From:** LNK (development on real hardware), for the maintainer · **To:** KRN (the kernel's ACPI, `init`)

## Why (the maintainer, 2026-11-08)

Test machines often have no monitor and no keyboard. Then the only way to turn one off is the power button, held for 4 s: the hardware's override, past MIND Core.

**What follows from it:**
- The next boot is a safe start. On the DEXP (`boot0008.log`) it left out `rtw88_wifi`, `wlan`, `bt_hci`, `audio_gw` and `video_gw`: `INIT] SAFE START: THE LAST RUN DID NOT END AS ASKED`.
- The machine came up with no network, so neither Effector nor the link could reach it.

**The maintainer asked** that the power button shut down cleanly.

## What exists

- **`kernel/src/arch/x86_64/acpi.rs`** does S5 power-off (000-KRN-0080) and the reset register, and nothing else.
- **No power-button event is read.** Neither the SCI nor PM1's `PWRBTN_STS` is handled, so a short press does nothing.

## Asked (yours to number)

1. **The kernel:**
   - **When.** When the FADT's flag `PWR_BUTTON` (bit 4) is clear, the machine has a fixed-feature power button.
   - **ACPI mode.** If `SCI_EN` in PM1 control is clear, the platform is switched to ACPI mode (`ACPI_ENABLE` to `SMI_CMD`, then wait for `SCI_EN`).
   - **The button.** `PWRBTN_EN` is set in PM1 enable, and the SCI (the FADT's `SCI_INT`, routed as the MADT says) is taken.
   - **GPEs.** Those this kernel does not handle stay disabled, so that the shared SCI does not storm.
   - **On a press:** `PWRBTN_STS` is cleared (write 1), and `init` is told.
2. **`init`:**
   - **The same path as the shell's `shutdown`:** services stopped, disks flushed, the run marked as ended as asked, so that the next boot is not a safe start; then S5.
   - **The log:** a line for the press and one for each step.
   - **A second press while it shuts down** changes nothing. The 4-second hold stays the hardware's.
3. **The hardware report** decodes the FADT for this:
   - `PWR_BUTTON` fixed or by control method, `SCI_EN` as found, and the PM1 event block.
   - A machine whose button is a control-method device (`\_SB.PWRB`, needing AML) is said to be one and left as today.

## Evidence

**The DEXP N100** (`hw0008.txt`, 2026-11-08, build `d83c98c5`):
- `profile desktop, flags 0x3C4E5, PM timer port 0x1808, SCI 9, reset register 0xCF9 value 0x6`;
- bit 4 of `0x3C4E5` is clear: a fixed-feature power button.

The Ryzen's and the MacBook Pro's flags are not read yet.

## How it can be tested

- **QEMU:** the monitor's `system_powerdown` raises this very event, on both `pc` and `q35`. So a CI suite can press the button and check the clean end, and that the next boot is not a safe start.
- **The DEXP:** a short press with no screen and no keyboard attached; the next boot's log shows no `SAFE START`. The machine and the build are named (MC-12.1).

Related: 000-KRN-0080 (power off on x86, done), [000-KRN-0076](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/main/issues/000-KRN-0076-a-frozen-machine-restarts-and-starts-safely.md) (the safe start).
