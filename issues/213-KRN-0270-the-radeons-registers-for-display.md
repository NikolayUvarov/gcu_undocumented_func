# 213-KRN-0270 — The Radeon's registers for `display`: a boot image and `init`'s grant, the firmware's state kept

**Type:** `init`, the boot images (handoff 38) · **Owner:** `KRN` (the kernel session), for DRV's 213-DRV-0086 · **Priority:** P1 · **Status:** in progress (made, 2026-11-21) · **Blocked by:** — · **Main task:** [213](213-amd-ryzen-pc.md) · **Constitution:** MC-3.12, MC-6.1, MC-12.1, MC-12.3

## Problem

DRV's [213-DRV-0086](213-DRV-0086-display-the-second-output-lit.md) lights the Ryzen's second monitor through a new service, `display`, for the Cezanne Radeon's display engine. Its step 1 reads the engine's registers and logs what the firmware left. DRV named what it needs of KRN on 2026-11-21 (`KRN/2026-11-21T1259Z-DRV-a-notice-the-ryzens-display-engine-a-grant-memory-and-the-compositor.md` on `internal-dev-messages`):
- the device for a new service;
- its register BAR;
- `display` in the boot image.

The maintainer asked the same day that the Ryzen's work go first, as the machine stays for two more days. So the grant is made before DRV's survey ([213-DRV-0085](213-DRV-0085-the-radeons-display-engine-surveyed.md)) has its facts. It follows what `amdgpu` does on every chip since Bonaire: the registers are BAR 5.

**What must not happen.** This device is also the one whose output the firmware's GOP set up, the screen MIND Core draws on. So:
- a grant that cleared its bus mastering, as the Realtek drivers' first grant does, could blank that output;
- a quiesce on a fault, which turns decoding off, would blank it for certain.

## Plan

1. **A boot image `display`** after `clipboard`, last before the shell (handoff 38: 53 images, `BootInfo` 1224 bytes). It comes after `effector`, as the gateway's lists hold the first 48 units.
2. **`init` finds `1002:1638` of class 03** and grants:
   - BAR 5 in `SLOT_DEV0`, with `PLATFORM_DEVICE_BAR`, which keeps the decoding and mastering the firmware set where they are all on;
   - the device's index as the argument.

   It logs the command register before and after. On any other machine, QEMU's among them, `display` is not started: `NO DEVICE`.
3. **`init` never stops the device:**
   - it records no device for `display`, so a fault, a restart or a safe start does not quiesce it;
   - `display` is among what a safe start leaves out (`SAFE_LEFT_OUT`, the eighth);
   - `data/services.txt` may turn it off.
4. **The image holds KRN's stub** until DRV's service replaces it. The stub logs the grant: the PCI identity, the command register, BAR 5's address and the size granted. It reads no register and writes none. `display/` is DRV's directory from then on.
5. **Later, with DRV's survey:**
   - memory to scan out from: BAR 0, the carve-out's aperture, would serve with no DMA grant;
   - the compositor's second screen.

   Each is a task of its own.

## Acceptance criteria

- [ ] Host: `tests/boot_abi_host.rs` (handoff 38, 53 images, `display` after `clipboard` and before the shell), `tests/runtime.rs`, `tests/safe_start_host.rs`.
- [ ] QEMU, x86 and aarch64: `display` is not started (`NO DEVICE`), and every other boot is as before; the gate's groups pass.
- [ ] **On the Ryzen** (DRV, MC-12.1):
  - `display` starts;
  - `init` logs the command register as the firmware left it, before and after the grant;
  - the stub logs `1002:1638` and BAR 5's size;
  - `DP-2` keeps MIND Core's screen.
- [ ] docs/profile/bootstrap.md and docs/api (EN) say what `display` holds.

**Not covered:** any register read or write, which are DRV's (213-DRV-0086); memory to scan out from; the compositor's part.

## Related

[213-DRV-0078](213-DRV-0078-a-display-driver-for-the-ryzens-radeon.md), [213-DRV-0085](213-DRV-0085-the-radeons-display-engine-surveyed.md), [213-DRV-0086](213-DRV-0086-display-the-second-output-lit.md), [213-DRV-0074](../issues-done/213-DRV-0074-the-ryzens-second-monitor.done), [000-KRN-0235](../issues-done/000-KRN-0235-init-starts-the-clipboard-service.done) (the last image added).
