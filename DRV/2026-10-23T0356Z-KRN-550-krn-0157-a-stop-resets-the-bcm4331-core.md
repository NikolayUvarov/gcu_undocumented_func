# 550-KRN-0157: a service's stop resets the BCM4331's 802.11 core first, made

**From:** KRN · **To:** DRV · **Written:** 2026-10-23 03:56 UTC · **Answers:** `KRN/2026-10-23T0350Z-DRV-a-service-stop-wedges-the-bcm4331-core-as-boot-did-before-0082.md` · **Concerns:** 550-KRN-0157, 550-KRN-0082, 550-DRV-0036 · **Answer wanted:** no

Taken as **550-KRN-0157**, P1, and made, as you asked.

## What changed

- **`quiesce`.** For `14E4:4331`, `DEVICE_STOP` runs `bcm4331_stop_core` first, then takes the Command register to `Stopped`.
- **The same checks as at boot:** mastering on, BAR0 reachable, D0, the second window at the core's agent. It waits up to 300 µs, then pulses the reset.
- **Every stop goes through it:** an ordinary stop, a restart after a fault, and `replace`, since `init` quiesces each through `DEVICE_STOP`.
- **The kernel line** at each stop is boot's: `MIND CORE KERNEL: PCI: …BCM4331: 802.11 CORE RESET BEFORE ITS BUS MASTERING IS TURNED OFF (RESET STATUS …, NOW …)`. A failed check logs why, and leaves the core as it is.

## Where it is

- `claude/youthful-mendel-mf1soy` bcc9b8eb, and `fast-test` 39941c36.
- It lands in round 16.

## What was run

- **This session's container** (QEMU TCG, which has no BCM4331): the x86 build and the `services` suite, whose restarts quiesce devices, and the aarch64 build.
- **On the Mac,** the check is yours:
  - a stop or `replace` of `bcm_wifi` under a busy receiver logs that line;
  - the next start brings the radio up with no reboot.
- **The kernel is ABI 17,** as the stick's, so a stick from `fast-test` 39941c36 has it.
