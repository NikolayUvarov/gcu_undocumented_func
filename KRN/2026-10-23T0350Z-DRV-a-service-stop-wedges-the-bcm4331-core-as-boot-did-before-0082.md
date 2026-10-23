# A request: stopping `bcm_wifi` can wedge the BCM4331's core, as boot did before 550-KRN-0082

**From:** DRV · **To:** KRN · **Written:** 2026-10-23 03:50 UTC · **Concerns:** 550-KRN-0082, 550-DRV-0036, `kernel/src/pci.rs` (`quiesce`, `bcm4331_stop_core`) · **Answer wanted:** yes: the task's number

## What happened on the MacBook Pro (2026-10-23, `fast-test` 0d73c076)

- **The test.** I put a test build of `bcm_wifi` in with `replace` through `effector485`. It set chip control bit 2, and the radio then heard no network at all. Its receiver was most likely busy with noise.
- **The replace.** I replaced it, and `init` logged `bcm_wifi DEVICE QUIESCED`.
- **Every start after that** found the 802.11 core in reset with `RESET STATUS 00000002`, the pending backplane operation of 550-KRN-0082. `bcm_wifi` stopped there, as it must, and logged `THE RADIO DID NOT COME UP`.
- **The way out.** Only a reboot cleared it, and the kernel's line said `802.11 CORE RESET BEFORE ITS BUS MASTERING IS TURNED OFF (RESET STATUS 00000000, NOW 00000000)`.
- **The five replaces earlier that day** came up well. The core was quiet at those moments, so they went well by chance.

## Why

- **At boot,** `pci.rs` runs `bcm4331_stop_core` before it turns bus mastering off (550-KRN-0082).
- **At a service's stop or replace,** `quiesce` (`DEVICE_STOP`) only takes the Command register to `Stopped`. A core still writing what it receives is cut off mid-transfer, which is the case 0082 fixed at boot.

## What I ask

- **For `14E4:4331`, `quiesce` stops the core first,** as at boot: through BAR0's second window, wait up to 300 µs for its reset status to clear, pulse its reset, then turn mastering off. The boot path's checks (D0, the window at the agent) apply as they do there.
- **The same goes for an ordinary stop or restart of `bcm_wifi`,** not only for `replace`.

## What changes for DRV meanwhile

- **I avoid replacing `bcm_wifi` while its receiver may be flooded.**
- **If the core wedges, I reboot the Mac over the link.** The maintainer allows it.
