# 308-APP-0122 — A recovery key shown once and typed back, and a store opened with one

**Type:** tools · **Owner:** `APP` track · **Priority:** P2 · **Status:** open · **Blocked by:** [308-KRN-0204](308-KRN-0204-a-store-client-that-may-recover-the-volume-key.md) (the store client badged `BADGE_RECOVER`, which `init` grants to the shell alone, and `REQUEST_RECOVER`, agreed with KRN on 2026-11-04), and `blockstore.wit` 2.4 (STO, [308-STO-0028](308-STO-0028-a-recovery-key-and-a-passphrase.md)) · **Main task:** [308](308-encryption-at-rest.md) · **Roadmap:** track B · **Constitution:** MC-6.8, MC-9.6, MC-11.9

STO's request of 2026-11-04 in [requests-APP.md](requests-APP.md), numbered here.

## Problem

Since 308-STO-0022 a medium's volume key is kept by `keystore`, so a medium moved to another machine opens nothing. The store's own part of a recovery key is built (308-STO-0028): `recovery-new`, `recovery-keep` and `recover`, and the key's written form (`mind::blockstore::recovery_text` and `recovery_parse`). Nothing yet shows the owner a key or takes one back.

The key must reach no log and no serial line. The console is copied to serial, which the tests read, so a console program such as `blocks` cannot show it.

## Plan

- **A program with a window of its own, `recovery`**, rather than a page of `wm`'s settings.
  - **Why a program.** It is the only one that asks for the recovery client: a request bit lent by the shell, asked of the user each time, as `REQUEST_FIRMWARE` is.
  - **What that saves.** `wm` holds many clients already; it need not hold this one. So `init` lends the client to the shell alone (KRN agreed, 2026-11-04: 308-KRN-0204, the bit `REQUEST_RECOVER`).
  - **The window.** The compositor draws it, and nothing of it goes to serial.
- **"Make a recovery key":**
  1. `recovery-new`;
  2. the key shown once in `recovery_text`'s 8 groups of 4, with a request to write it down;
  3. the key asked for again, without echo, and given to `recovery-keep`.

  Until `recovery-keep`, the older keys still open the medium. A key typed back wrong is told, and asked for again; closing the window leaves the older keys as they are.
- **"Open with a recovery key"**, for a store not mounted with `key`:
  1. the key typed without echo;
  2. checked with `recovery_parse`, so a typo is told before the store is asked;
  3. `recover`.
- **Without a screen**, for the tests and the MacBook Pro: `--key-file <path>`, as `wifi join --passphrase-file` does (550-APP-0078). The file's first line names the medium's id. With `new`, the key is written to that file instead of shown; with `open`, it is read from it.
- **What the program prints on its console** names the step and its result only, never the key.

## Acceptance criteria

- Host tests: the key's groups as shown, a typo refused before the store is asked, the two steps of "make" (a key typed back wrong keeps the older keys), the key file's form.
- QEMU, x86 and aarch64, with `--key-file`:
  - a key made and kept;
  - the medium on a machine whose `keystore` does not hold its key stays closed, and the key file opens it;
  - the key's text appears nowhere on serial.
- docs/tools (English and Russian).

## Related

[308](308-encryption-at-rest.md), [308-STO-0028](308-STO-0028-a-recovery-key-and-a-passphrase.md), [308-KRN-0204](308-KRN-0204-a-store-client-that-may-recover-the-volume-key.md), 550-APP-0078 (`--passphrase-file`).
