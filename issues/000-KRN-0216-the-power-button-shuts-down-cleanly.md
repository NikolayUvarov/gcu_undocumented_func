# 000-KRN-0216 — A short press of the power button shuts the machine down cleanly (x86, a fixed-feature button)

**Type:** kernel (the ports the FADT names), `init` · **Owner:** kernel session · **Priority:** P1 · **Status:** in progress (made and tested in QEMU on x86; the gate and the DEXP's run left) · **Blocked by:** — · **Roadmap:** track A · **Constitution:** MC-6.1, MC-6.3, MC-12.1, MC-12.3

## Problem

Asked by LNK for the maintainer (`KRN/2026-11-08T1346Z-LNK-the-power-button-shuts-down-cleanly.md` on `internal-dev-messages`).

- **Test machines often have no screen and no keyboard.** Then the only way to turn one off is the power button held for 4 s: the hardware's override, past MIND Core.
- **What follows from that:** the next boot is a safe start (000-KRN-0076). On the DEXP N100 (`boot0008.log`, build `d83c98c5`) it left out `rtw88_wifi`, `wlan`, `bt_hci`, `audio_gw` and `video_gw`. The machine came up with no network, so neither Effector nor the link could reach it.
- **Today a short press does nothing:**
  - `kernel/src/arch/x86_64/acpi.rs` does S5 (000-KRN-0080) and the reset register;
  - nothing reads the SCI or PM1's `PWRBTN_STS`;
  - the kernel lends no ACPI fixed-hardware port: `platform::ports` holds the platform profile's legacy ranges and the ECDT's two (211-PRT-0008).
- **The run's mark** (000-KRN-0076) is cleared by `init` a minute past `READY`, and by the kernel at every halt, restart and power-off asked through `REBOOT` or `HALT`. So a shutdown asked of the system never leaves it; a cut power before that minute does.

**The DEXP's FADT** (`hw0008.txt`): flags `0x3C4E5`, so bit 4 (`PWR_BUTTON`) is clear: a fixed-feature power button; SCI 9.

## Plan

**No ABI change.** As the ECDT's ports were for the battery service, the kernel lends the ports the FADT names, and `init` does the rest with its platform privilege, before it drops it at `READY`.

1. **The kernel:**
   - `platform::ports` also allows each block the FADT names in system I/O space:
     - `SMI_CMD` (one byte);
     - `PM1a_EVT_BLK` and `PM1b_EVT_BLK` (`PM1_EVT_LEN`);
     - `PM1a_CNT_BLK` and `PM1b_CNT_BLK` (`PM1_CNT_LEN`);
     - `GPE0_BLK` and `GPE1_BLK` (their lengths).

     The `X_` generic addresses win over the 32-bit fields when they are in I/O space.
   - **The hardware report's FADT line** says, after what it shows now:
     - `PWR_BUTTON` fixed or by a control method;
     - `SCI_EN` as found;
     - the PM1 event block and its length.
2. **`init`, at boot** (x86, when the FADT has a fixed-feature button and a PM1a event block in I/O space):
   - **ACPI mode:** if `SCI_EN` in PM1a control is clear, `ACPI_ENABLE` is written to `SMI_CMD` and `SCI_EN` waited for (1 s at most). If it does not come, the button is left as today, with a line.
   - **Every GPE disabled** (their enable halves written 0, their status cleared), and every fixed event but the power button. So the shared SCI does not storm with events nothing here handles.
   - **`PWRBTN_STS` cleared** (written 1), then `PWRBTN_EN` set.
   - **The SCI line taken** (`PLATFORM_IRQ`, an ISA line) and bound to `init`'s own endpoint.
   - **One line:** `[INIT] THE POWER BUTTON SHUTS THE MACHINE DOWN (SCI n, PM1 EVENTS AT 0x…)`, or why not:
     - a button by control method (`\_SB.PWRB`, which needs AML);
     - no PM1a event block in I/O space;
     - an SCI past line 15;
     - ACPI mode not reached.
3. **`init`, on a press:**
   - `PWRBTN_STS` is cleared and the line acknowledged;
   - `[INIT] THE POWER BUTTON WAS PRESSED: SHUTTING DOWN`;
   - the same path as `init`'s restart for the updater: the services told and stopped in reverse start order, the volumes written;
   - then the machine is turned off (S5) through `REBOOT`, which `init`'s restart privilege may ask: the kernel clears the run's mark there, so the next boot is not a safe start.
   - **A second press** while it shuts down changes nothing. The 4-second hold stays the hardware's.
4. **A check in QEMU** (x86): QEMU's monitor `system_powerdown` raises this very event.
   - With the machine up less than a minute past `READY`, a press: the lines above, QEMU ends by itself (S5).
   - The same disk booted again: no `SAFE START` line.

## Acceptance criteria

- **The QEMU check passes on x86** in the gate: a press shuts the machine down, and the next boot is not a safe start.
- **A machine whose FADT names no fixed-feature button** says why and works as before.
- **On the DEXP:** a short press with no screen and no keyboard attached; the next boot's log shows no `SAFE START`. That is evidence for that machine and build only (MC-12.1).

## Not in this task

- **aarch64:** QEMU's `virt` raises its button through ACPI's GED (hardware-reduced, by AML) or a device tree's `gpio-keys`. Neither is read here.
- **A button by control method,** and the sleep button.
- **A narrower safe start** after a cut power: 000-KRN-0217.

## Progress (2026-11-08)

On `claude/youthful-mendel-mf1soy`:
- **`common/fadt.rs`:** one pure parser of the FADT's fixed hardware, included by the kernel and `libmind`. It reads the blocks, the SCI and the flags; the generic addresses win over the 32-bit fields. Host tests in `tests/fadt_host.rs` (5): QEMU's table, the DEXP's flags, the generic addresses, and what an untrusted table cannot make it accept.
- **The kernel lends only the PM1 event blocks,** not `SMI_CMD`, PM1 control or the GPE blocks the plan named. Writing `SMI_CMD` runs the firmware's SMM handlers with any value, and PM1 control turns the machine off, which `REBOOT` already does.
  - Before the first lending it puts the chipset in ACPI mode (`ACPI_ENABLE` to `SMI_CMD`, as `power_off` does) and disables every GPE and fixed event, once.
  - It says which on the console: `SWITCHED TO ACPI MODE` or `IN ACPI MODE ALREADY`, then `EVERY GPE AND FIXED EVENT DISABLED, THE PM1 EVENTS LENT`.
- **`REBOOT` names its caller on the console:** `MIND CORE KERNEL: init (PID 1) ASKED TO TURN THE MACHINE OFF` (or `TO RESTART THE MACHINE`). It is the evidence left once the system log is gone.
- **`init`:**
  - takes the blocks and the SCI's line before it drops the platform privilege, binds the line to its endpoint, clears `PWRBTN_STS` and sets `PWRBTN_EN`;
  - on a press, its restart path with `POWER-OFF` as the stop notice's reason and S5 at the end. The kernel clears the run's mark in `REBOOT`.
- **The hardware report's FADT line** adds the button's kind, `SCI_EN` as found and the PM1 event blocks.
- **The `power` suite** (x86), in the group `x86: boot, display, shell, memory, clock`, on QEMU's `pc` (PIIX4, PM1 events at `0xB000`) and `q35` (ICH9, at `0x600`):
  - a reset before a minute past `READY` gives a safe start, so the run's mark is kept across it;
  - in that boot, a file is written and the button pressed. `init` asks for S5 0.7 s after the press;
  - the boot after it is no safe start, the file is there and its volume was not left dirty.
  - Passed in this session's container: 52 s for both machines.
- **Seen in QEMU:** line 9 is held by `init` alone on both machines, with no SCI before the press, since the devices there use MSI. On a machine whose devices share the SCI's line, `init` takes each of their interrupts too, reads PM1's status and acknowledges.

## Related

[000-KRN-0076](000-KRN-0076-a-frozen-machine-restarts-and-starts-safely.md) (the safe start), 000-KRN-0080 (power off, done), 211-PRT-0008 (the ECDT's ports), [000-KRN-0217](000-KRN-0217-a-safe-start-that-leaves-out-only-what-it-suspects.md).
