# 308-APP-0130 — The shell asks for `keystore`'s passphrase at boot, and sets or changes it

**Type:** tools (`shell`) · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open (waits for the badge) · **Blocked by:** [308-KRN-0205](308-KRN-0205-the-boot-waits-for-keystores-passphrase.md) (the shell's `keystore` client badged `BADGE_KEY_PASSPHRASE`, and the shell started before the services that need the device key) · **Main task:** [308](308-encryption-at-rest.md) · **Roadmap:** track B · **Constitution:** MC-4.2, MC-11.5, MC-11.9

**Asked by:** NET (made by STO), 2026-11-10, in [requests-APP.md](requests-APP.md), for [308-NET-0055](308-NET-0055-keystores-passphrase-without-a-tpm.md). Numbered on 2026-11-10.

## Problem

A machine without a TPM keeps its device key unsealed on the boot disk, as no task can type `keystore`'s passphrase. `keystore.wit` 1.6, in `main` since round 80, has:
- `key-state`, any client's, answered at once: `held`, `waiting` or `none`;
- `unlock(passphrase)` and `passphrase-set(old, new)`, for badge 6 alone. A passphrase is 8 to 128 bytes;
- `denied` for a wrong one. From the third wrong one in a row, `budget` until a wait has passed, doubling from 1 s up to 60 s.

## Plan

- **At boot,** while `key-state` says `waiting`:
  - the shell says in one line why the boot waits;
  - it reads the passphrase without echo, as a console program's secret line is read (000-APP-0126, `shell/src/secret.rs`), and calls `unlock`;
  - on `denied` or `budget` it says so, and the wait, and asks again.
- **A command to set or change it** (`passphrase`):
  - it asks for the old one (none the first time) and the new one twice, each without echo, then calls `passphrase-set`;
  - asked at the console alone: no client of the shell's commands, and no script.
- **No passphrase on a command line, in the history or in a log:**
  - the shell's copies are wiped once sent;
  - the generated client wipes its call buffer (000-KRN-0226).

## Acceptance criteria

- **Host tests:** the lines, the two new ones that differ, a passphrase out of 8 to 128 bytes.
- **A suite that types it over the serial console** with the badge from 308-KRN-0205:
  - the boot waits, and a wrong passphrase is refused, then the right one unlocks;
  - a change takes the old one and the new one twice;
  - the passphrase is in no serial line, log or history.

## Related

[308-NET-0055](308-NET-0055-keystores-passphrase-without-a-tpm.md), [308-KRN-0205](308-KRN-0205-the-boot-waits-for-keystores-passphrase.md), [000-APP-0126](../issues-done/000-APP-0126-a-console-program-reads-a-secret.done), [308-APP-0122](308-APP-0122-a-recovery-key-shown-once-and-typed-back.md).
