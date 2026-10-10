# 211-KRN-0058 — Fixed slots for the shell's command endpoint and the clipboard (ABI 5)

**Type:** kernel (ABI) · **Owner:** kernel session · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Roadmap:** track G, for main task [211](211-intel-pc-from-a-sata-ssd.md) and the tools track's 211-APP-0040 and 000-APP-0032 · **Constitution:** MC-11.1 (authority only through held capabilities), MC-12.4 (an interface change gets a new version), MC-12.7 (an explicit transition)

## Problem

The tools track asked for two slots in `requests-KRN.md` (2026-10-09; the requests are in its branch):

- **`SLOT_SHELL`.** The shell will serve `idl/shell.wit`, and `console` in `wm` sends it the shell's commands (211-APP-0040). The endpoint goes from the shell to `wm` and on to `console` in launch sessions, so it needs a fixed slot.
- **`SLOT_CLIPBOARD` and `REQUEST_CLIPBOARD`.** These are for the clipboard service (000-APP-0032).

The application slots 1–29 are all named. Fixed slots end at `SLOT_DYNAMIC` (30), and the kernel refuses a spawn grant at or above it. So a new slot moves `SLOT_DYNAMIC`, and a program built for the new layout does not start on a kernel built for the old one. That is an interface change: a new ABI version (MC-12.4).

The maintainer's run on the MacBook Pro (2026-10-10) needs the first one. There `console` in `wm` could not reboot, kill, read the log or reach the network: only the shell holds those. The tools track does the rest ([requests-APP.md](requests-APP.md), "The camera from `wm`, and the shell in a window").

## Plan

- **`common/abi.rs`.**
  - `SLOT_SHELL` = 30 and `SLOT_CLIPBOARD` = 31.
  - `SLOT_DYNAMIC` = 32.
  - `ABI_VERSION` 5.
  - `LAUNCH_SLOTS`: the fixed slots a launcher may fill, which the loader checks against. It was a list inside `loader`.
- **`libmind::process`.** `REQUEST_CLIPBOARD` (bit 19). `REQUEST_SHELL` comes with the tools track's change, as its request says; bit 20 is the next free one.
- **`loader`** accepts both slots in `grant`.
- **Not here.**
  - `init` starting `clipboard` waits for the service, which the tools track writes.
  - The shell's endpoint is made and lent by the shell itself.
- **Docs.** `docs/api` gets the slots and the version.

## Acceptance criteria

1. **A host test of the layout** (`tests/runtime.rs`):
   - the named application slots are distinct and below `SLOT_DYNAMIC`;
   - `LAUNCH_SLOTS` holds `SLOT_SHELL` and `SLOT_CLIPBOARD` and none of the loader's own slots 2–6;
   - `SLOT_DYNAMIC` leaves dynamic slots in the initial capability space.
2. **The ABI version.** Every program and the kernel build for ABI 5. The loader's ABI check (`loader-abi-test`) still refuses a kernel of another version.
3. **The gate.** All suites pass on x86 and aarch64: the isolation, heap and SMP suites use `SLOT_DYNAMIC` by name.
4. **End to end.** A launcher filling `SLOT_SHELL` in a launch session and the program holding the endpoint there is checked by the tools track's `wm` suite with 211-APP-0040, the first launcher to fill it.

## Related

[072](../issues-done/072-fixed-grant-slots.done) (fixed grant slots), [109-KRN-0042](../issues-done/109-KRN-0042-parser-service-at-boot.done) (`SLOT_PARSE`, the same pattern), [requests-KRN.md](requests-KRN.md).
