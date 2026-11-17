# 000-APP-0142 — Two checks wait for what a loaded host gives late: the Sound page's whole line, and `svc`'s file write

**Type:** tests (`tests/qemu_smoke.py`) · **Owner:** tools track (`APP`) · **Priority:** P1 (the gate's rounds carry 801's work) · **Status:** open · **Blocked by:** — · **Main task:** — · **Roadmap:** — · **Constitution:** MC-12.1, MC-12.2

**Asked by:** KRN, the integrator, on 2026-11-15, in two requests in [requests-APP.md](requests-APP.md), with messages on `internal-dev-messages`:
- `APP/2026-11-15T1701Z-KRN-the-sound-page-check-reads-its-line-before-it-is-whole.md`;
- `APP/2026-11-15T2321Z-KRN-the-svc-check-waits-8-s-for-a-write.md`.

Numbered on 2026-11-17.

## Problem

- **The Sound page check** (`sound_meter_check`, for 000-APP-0077) failed round 142's gate on i9.
  - It waited for the start of `[WM] SOUND PAGE Microphone: peak …`, then read the whole line at once.
  - The end of the line, `noise floor … dBFS`, was not there yet: `re.search` gave `None`.
- **The `svc` check** (`svc_boot_check`, for 173-APP-0092) failed round 143's gate on x32, beside another lane.
  - `svc after windows sysmon` writes `data/services.txt`. The write had not reached the disk in the harness's 8 s.
  - The stuck record showed `svc` waiting for `vfs_server`, `vfs_server` for `ata`, and `ata` running.
- Both passed alone. A red round returns nobody's request but delays everyone's, 801's among them.

## Plan

- **The Sound page check** waits until the line matches whole, 30 s at most, then reads it.
- **`WRITE_WAIT` (30 s)** for a command that writes a file on the boot disk, as `DMESG_WAIT` is for `dmesg`. The wait ends at the prompt, so an unloaded run takes no longer.
  - It is given to `svc disable`, `svc after`, `svc enable` and `svc reset`, the ones that write.
  - It is given as well to the 801 checks' writes that wait 8 s today: `mode remote-update off` and `effector policy allow update everything` / `deny reboot`.

## Acceptance criteria

- The `audio`, `tools` and `modes` suites pass on x86 with the change.
- Evidence: this container; no claim about a loaded host beyond what the gate then shows (MC-12.1).

## Related

[000-APP-0077](000-APP-0077-microphone-sensitivity.md), [173-APP-0092](../issues-done/173-APP-0092-svc-boot.done), [801-APP-0139](../issues-done/801-APP-0139-mode-remote-update-in-the-shell.done), [801-APP-0140](../issues-done/801-APP-0140-effector-policy-in-the-shell.done).
