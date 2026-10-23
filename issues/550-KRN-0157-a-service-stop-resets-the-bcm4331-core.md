# 550-KRN-0157 — A service's stop resets the BCM4331's 802.11 core first, as boot does

**Type:** kernel (`pci.rs`, `quiesce`) · **Owner:** kernel session · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Main task:** [550](550-network-on-real-hardware.md) · **Constitution:** MC-1.5, MC-6.8

From DRV's request of 2026-10-23 (`KRN/2026-10-23T0350Z-DRV-a-service-stop-wedges-the-bcm4331-core-as-boot-did-before-0082.md` on `internal-dev-messages`).

## Problem

- **What happened on the MacBook Pro** (`fast-test` 0d73c076).
  - A test build of `bcm_wifi` left the radio's receiver busy with noise.
  - It was then replaced; `init` logged `bcm_wifi DEVICE QUIESCED`.
  - Every start after that found the 802.11 core in reset with `RESET STATUS 00000002`, the pending backplane operation of [550-KRN-0082](../issues-done/550-KRN-0082-the-bcm4331-core-stopped-before-its-bus-mastering.done).
  - Only a reboot cleared it.
- **Why.**
  - At boot, `pci.rs` resets the core (`bcm4331_stop_core`) before it turns bus mastering off.
  - A service's stop, restart or replace (`DEVICE_STOP`, `quiesce`) only took the Command register to `Stopped`. That cut off a core still writing what it receives, the case 0082 fixed at boot.

## Plan

- **`quiesce` for `14E4:4331`** runs `bcm4331_stop_core` first, then takes the Command register to `Stopped`.
  - The reset goes through BAR0's second window.
  - It waits up to 300 µs for the reset status to clear, then pulses the core's reset.
  - It has the boot path's checks: mastering on, BAR0 reachable, the function in D0, the window at the core's agent. Each failed check is logged and leaves the core as it is.
- **The same path for every stop:** an ordinary stop, a restart after a fault, and a `replace` all quiesce the device through `DEVICE_STOP`.

## Acceptance criteria

- [ ] QEMU (no BCM4331 there): every suite passes as before (the gate).
- [x] On the MacBook Pro:
  - a stop or `replace` of `bcm_wifi` while its receiver is busy logs the kernel's `802.11 CORE RESET BEFORE ITS BUS MASTERING IS TURNED OFF`;
  - the next start brings the radio up with no reboot.
  - This is evidence for that machine only (MC-12.1). DRV's to run.

## Progress (2026-10-23)

- **Made** in bcc9b8eb. The x86 build and the `services` suite passed in this session's container, and the aarch64 build too.
- **DRV's run on the MacBook Pro** (`KRN/2026-10-23T0645Z-DRV-550-krn-0157-checked-on-the-macbook.md`):
  - **Before,** EFF's slot A with no 0157: three `replace`s of `bcm_wifi` under the Effector agent's traffic each left the core at `00000002`, and only a reboot cleared it.
  - **After,** slot B with `pci.rs` from bcc9b8eb: three `replace`s under traffic, 25 s apart. Each time the next start found `RESET STATUS 00000000` before and after, and `wlan` joined again 10 to 13 s later.
  - **The kernel's line** at a stop goes to its own console only, not to the system log read over the link. The core's state shows the stop worked.
  - This is evidence for that machine and those builds only (MC-12.1).
- **Left:** the gate.

## Related

[550-KRN-0082](../issues-done/550-KRN-0082-the-bcm4331-core-stopped-before-its-bus-mastering.done), 550-DRV-0036, `kernel/src/pci.rs` (`quiesce`, `bcm4331_stop_core`).
