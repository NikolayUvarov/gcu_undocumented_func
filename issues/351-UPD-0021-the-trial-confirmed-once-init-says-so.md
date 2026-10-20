# 351-UPD-0021 — A trial confirmed on the disk once `init` says it confirmed it

**Type:** service · **Owner:** `UPD` track · **Priority:** P2 · **Status:** open (built; the `update` suite's check runs with the next gate) · **Blocked by:** — (000-KRN-0102, `init.wit` 1.4, on `main` 0429ea3) · **Main task:** [351](351-self-update.md) · **Roadmap:** track C · **Constitution:** MC-9.3, MC-9.5, MC-12.3

From the kernel track's note "`init` says how the boot went (000-KRN-0102)" (`UPD/2026-10-20T0616Z-KRN-init-says-how-the-boot-went.md`, first written into `requests-UPD.md` on the kernel branch on 2026-10-14). It is 000-KRN-0102's second criterion.

## Problem

- **What the updater did.** On a trial boot it waited until 5 s past the kernel's deadline (120 s), then wrote the confirmed record: past the deadline, a boot that was not confirmed would have restarted the machine.
- **What that cost.** The other slot stayed untouched, and nothing ran automatically, for two minutes after every update. The QEMU `update` check waited that long.
- **What changed.** `init.wit` 1.4 `boot()` says the slot, whether it is a trial, and whether `init` confirmed it. `init` answers once it has confirmed or refused the boot.

## Plan

- **The ask.** Every second while a trial waits, the updater asks `init` through the client it already holds (`SLOT_LIFECYCLE`). Each call waits 1 s at most.
- **Confirmed.** It writes the confirmed record at once: `[UPDATER] SLOT … CONFIRMED ON THE DISK: INIT CONFIRMED THE TRIAL BOOT`.
- **Refused.** It writes nothing and does nothing automatic. The kernel restarts the machine at its deadline, into the other slot, which stays untouched.
- **No answer it can use** (an `init` before 1.4, or a refusal of the call): it keeps to the deadline as before, and says so.
- **The `update` suite** requires the line above within 90 s, instead of waiting 240 s past the deadline.
- `docs/update/updater.md` 0.2, in English and Russian.

## Acceptance criteria

- The `update` suite passes on x86 and aarch64 with the confirmation from `init`.
- The deadline path stays for an `init` that cannot say. This is read from the code: no suite runs an older `init`.

## Related

000-KRN-0102, [351-UPD-0007](../issues-done/351-UPD-0007-updater-service.done), 351-KRN-0014.
