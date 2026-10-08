# 600-APL-0010 — `03_run_qemu_aarch64.sh` on macOS: the screen in a window, Bash 3.2, a choice of accelerator

**Type:** run script · **Owner:** `APL` track (open) · **Priority:** P2 · **Status:** open · **Blocked by:** — for the change; the check on macOS needs a Mac ([issues-human](../issues-human/README.md#4-a-mac-with-apple-silicon-to-test-on)) · **Roadmap:** track H · **Constitution:** MC-12.1, MC-12.9

Part of main task [600](600-apple-silicon-mac-vm-host.md).

## Problem

On an Apple Silicon Mac the script (expected; not yet run on a Mac):

- opens no window: it shows the screen only where `DISPLAY` or `WAYLAND_DISPLAY` is set, which macOS does not set, and otherwise passes `-display none`; the guide adds `-display cocoa` by hand;
- under macOS's Bash 3.2 stops with `unbound variable` when it expands an empty array under `set -u`: `NET=()` with `MIND_NET=none`, and `DISPLAY_ARGS=()` when XQuartz sets `DISPLAY`;
- always runs HVF with `-cpu host`, so the configuration CI tests (TCG, `-cpu max`, with RNDR) cannot be run on the same Mac for comparison;
- sets `highmem=on` above `3G`, which may not fit the guest address space HVF gives on an M1 (expected to be 36 bits; not checked).

## Plan

- On macOS, QEMU's own window (Cocoa) by default; `MIND_DISPLAY=none` still leaves it out.
- Empty arrays expanded in a form Bash 3.2 accepts.
- `MIND_ACCEL=tcg` or `hvf` to choose; HVF stays the default on an Apple Silicon Mac.
- Above `3G` under HVF: see what QEMU does on an M1, then refuse with a clear message, or state the limit in the guide.
- Linux and WSL unchanged. The script is the porting track's (issues 204, 205) with the tools track's HVF lines (commit f28467a): `PRT` reviews the change.

## Acceptance criteria

On an Apple Silicon Mac: `./03_run_qemu_aarch64.sh` shows the system's screen in a window and the console on the terminal; `MIND_NET=none` and `MIND_ACCEL=tcg` work under macOS's own Bash; what happens above `3G` is stated in the guide. Runs on Linux are unchanged.

## Related

[600](600-apple-silicon-mac-vm-host.md), [600-APL-0011](600-APL-0011-first-run-on-a-mac.md), [docs/apple-silicon.md](../docs/apple-silicon.md) (section 3).
