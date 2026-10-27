# 211-DRV-0040 — `usb_host` gives an xHCI controller more than 32 scratchpad pages: the DEXP's asks 34

**Type:** driver · **Owner:** `DRV` · **Priority:** P1 · **Status:** in progress (2026-10-27) · **Blocked by:** — · **Main task:** [211](211-intel-pc-from-a-sata-ssd.md) · **Constitution:** MC-10.2, MC-12.1

## Problem

The maintainer booted a stick written from `fast-test` on a DEXP desktop (2026-10-27): no keyboard, no mouse in any port, and `effector485` on COM22 heard nothing from the machine's converter. The screen showed:

```
[USB] 34 SCRATCHPAD PAGES NEEDED, 32 AVAILABLE
[USB] CONTROLLER DID NOT START
[INIT] usb_host QUARANTINED: 3 RESTARTS IN 60 S
```

- **The controller wants 34.** An xHCI controller names in HCSPARAMS2 how many scratchpad buffers it needs (Max Scratchpad Buffers, up to 1023), and it does not run without them.
- **`usb_host` gives at most 32.** Its 512 KiB DMA region keeps 32 pages for them (0x10000 to 0x30000), so it refuses a controller that asks more.
- **Without that controller, the machine has no USB:** no keyboard or mouse, and no USB-serial converter for the link.

## Plan

1. **Pages beyond 32 come from the pool** (done). They are taken from the 64-page pool of device contexts and transfer rings, at most half of it, so a controller may ask up to 64.
   - The pages taken are never handed out or released.
   - The 32 left serve a keyboard, a mouse, a converter and a stick with room to spare: a device takes its output context and a page for each ring.
   - It logs `[USB] 34 SCRATCHPAD PAGES, 2 OF THEM FROM THE POOL`.
2. **A controller that asks more than 64** is still refused, with its number in the log.
3. **The QEMU suites** that run through `usb_host` (`usb`, `tablet`, `link`) check that nothing changed for a controller with none: QEMU's xHCI asks 0.
4. **On the DEXP:** the keyboard, the mouse and the link's converter work.

## Acceptance criteria

- [x] The QEMU `usb`, `tablet` and `link` suites pass (x86, 2026-10-27, on the merge of `main` 417a2f31 with this change).
- [ ] On the DEXP, `usb_host` logs the 34 pages with 2 from the pool, the controller starts, and the keyboard, the mouse and `effector485` through the machine's converter work. Evidence for that machine only (MC-12.1).

## Related

[211-DRV-0035](211-DRV-0035-superspeed-hubs.md), [211-DRV-0008](211-DRV-0008-usb-mouse-on-real-hardware.md), 800-DRV-0034.
