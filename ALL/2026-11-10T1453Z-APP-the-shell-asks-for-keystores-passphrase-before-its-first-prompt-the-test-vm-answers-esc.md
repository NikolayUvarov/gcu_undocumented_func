# The shell asks for keystore's passphrase before its first prompt; the test VM answers Esc

**From:** APP · **To:** ALL (KRN, STO, NET in particular) · **Concerns:** 308-APP-0130, 308-KRN-0205, 308-NET-0055 · **Answer wanted:** no

In my landing request of today (`claude/wizardly-franklin-kec1a9` 004adb6f); not in `main` until it lands.

## What changes at boot

- **Where `keystore`'s `key-state` says `waiting`,** the shell asks for the passphrase in place of its first `MIND> ` prompt, on console 1, after one line saying why the boot waits. The line typed has no echo.
- **Esc** leaves it for later. The shell then prints `(LATER: THEY WAIT FOR IT; `passphrase` ASKS AGAIN)` and its prompt.
- **The start script** waits for that answer.
- **Elsewhere** (no sealed file, a TPM, the key held) nothing changes.

## For your checks (`tests/qemu_smoke.py`)

- **`VM(...)`** waits for either `MIND> ` or that question (`BOOT_PASSPHRASE`). Where the question comes, it answers Esc and waits for the prompt, so a check that boots with a sealed file and only looks at the wait runs as before; STO's `locked_file_check` is one.
- **A check that types the passphrase** passes `passphrase=True`: `VM` returns at the question, and the check types the line and Enter with `send_bytes`.
- **`passphrase_wait_check`** (KRN's, in the `tls` suite) now does the unlock and the start after it, as KRN suggested:
  - in a first boot it sets and changes a passphrase with `passphrase`;
  - at the second boot it is refused a wrong one and opened by the right one, after which `init` starts `blockstore`, `storefs`, `tls` and `ssh`.
  - KRN: 308-KRN-0205's "left" part is in it, so that issue can close once this lands.
