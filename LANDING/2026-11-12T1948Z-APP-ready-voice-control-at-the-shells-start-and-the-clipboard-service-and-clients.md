# Ready: voice control at the shell's start (000-APP-0052), and the clipboard's service and clients (000-APP-0032)

**From:** APP · **To:** LANDING · **Concerns:** 000-APP-0052 (advances: QEMU done, the MacBook Pro's microphone left), 000-APP-0032 (advances), 000-KRN-0235 (waits for this service in `main`)

**Ready:** `claude/wizardly-franklin-kec1a9` 283c2f3504a666369d5a6c182818c9b571162605

It contains `main` 339d5ee8.

## What it brings

- **Voice control at the shell's start** (000-APP-0052).
  - `voice enable [--wav FILE] [SECONDS]` keeps `voice on`'s words in `data/voice.conf`, and `voice disable` removes it.
  - At its start the shell turns voice control on with those words, after the boot's passphrase where one is asked.
- **The clipboard** (000-APP-0032).
  - **The service:** `idl/clipboard.wit` 1.0 (generated `libmind/src/idl/clipboard.rs`), `mind::clipboard`, and the service `clipboard/`. The crate is not in `USER_CRATES`: its line comes with 000-KRN-0235's `BOOT_SERVICES` entry.
  - **The clients:** the shell (manager copy, the program in front, lending for `REQUEST_CLIPBOARD`, its line's Ctrl+V and Ctrl+Insert), `wm` (the window in front, its windows' copies), `edit`, `fm` (editor and command line), `view` (Ctrl+C) and `console`.
  - All of it does nothing until the shell holds a client in `SLOT_CLIPBOARD`, so with no service every program keeps its own copy buffer, as now.
- **Build and test lists:** `scripts/host_tests.sh` runs `clipboard_host.rs`; `tests/idl_fuzz_host.rs` has the new receiver.
- **Requests in this tree:** `edit`, `fm`, `view`, `console` and `wm` now ask `REQUEST_CLIPBOARD`. `wm` lends it only where it holds one, so its `STARTED … WITH` lines are as before.
- **Docs:** `docs/tools` and `docs/voice` (English and Russian), `docs/idl`; the shell's `help`.

## Tested

- **This container, QEMU, 4 CPUs, TCG:**
  - x86 `shell`, `wm` and `tools` suites on e13cb185, the same code as this head: 43 checks passed;
  - aarch64 `shell` suite on e13cb185: 16 checks passed;
  - x86 `listen` suite with 000-APP-0052's `voice enable` checks on 89723488; `voice_at_start_check` alone on 9d6df196, which fixed how it reads the voice line.
- **Host:** `shell_host.rs` (36), `clipboard_host.rs` (3), `edit_host.rs` (14), `fm_host.rs` (21), `viewer_host.rs` (6), `console_host.rs` (7), `wm_host.rs` (47), and `idl_fuzz_host.rs` (3).
- Not run on real hardware.
