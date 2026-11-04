# 111-KRN-0206 — `timed` among the boot services, with its three grants (handoff 29)

**Type:** `init`, the loader's handoff, `vfs_server` · **Owner:** kernel session · **Priority:** P2 · **Status:** in progress · **Blocked by:** — · **Main task:** [111](111-network-time.md), for [111-NET-0057](111-NET-0057-timed-sets-the-clock-by-sntp-within-bounds.md) · **Constitution:** MC-3.1, MC-11.4, MC-11.6, MC-12.4

## Problem

`timed` (111-NET-0057) runs as a boot service. It needs a flow grant for its servers, a clock that it may set, and a place for the floor that nothing else can move.

## Plan

- **A boot image after `netpolicy`.** That is handoff 29: the images after it are renumbered, and BootInfo gains one record. A loader of handoff 28 is refused by name (MC-12.4). The system calls' ABI is unchanged.
- **`init` grants it three things, and nothing else (MC-11.4):**
  - the flow the policy gives program `timed`, in `SLOT_NETWORK`;
  - the `rtc` client with the setting badge, in `SLOT_RTC`;
  - a `vfs_server` client badged `BADGE_TIMED`, in `SLOT_VFS`.
- **Not started without a time server.** When the policy gives `timed` no flow, init does not start it and logs `[INIT] timed NOT STARTED: THE POLICY GIVES IT NO TIME SERVER`.
- **Its private directory.** `system/timed` is a private directory of the boot volume, which `BADGE_TIMED` alone opens. No program, the shell included, can raise the floor and so make every correct time refused.
- **`data/services.txt`** may turn it off. It needs `netpolicy`.

## Acceptance criteria

- **Host tests:**
  - `tests/boot_abi_host.rs`: handoff 29, 48 images, `timed` after `netpolicy`;
  - `tests/vfs_zone_host.rs`: `system/timed` writable by `BADGE_TIMED` alone, and that badge reaching nothing else;
  - `tests/services_host.rs`.
- **QEMU, x86 and aarch64** (`timed_check`, 111-NET-0057):
  - it is started with a time line and sets the clock;
  - with none, the line above and no `timed` running.

## Related

[111-NET-0057](111-NET-0057-timed-sets-the-clock-by-sntp-within-bounds.md), [111](111-network-time.md).
