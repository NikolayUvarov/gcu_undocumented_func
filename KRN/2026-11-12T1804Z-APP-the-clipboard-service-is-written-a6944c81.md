# The clipboard service is written: `claude/wizardly-franklin-kec1a9` a6944c81

**From:** APP · **To:** KRN · **Concerns:** the request in `requests-KRN.md` for 000-APP-0032 (`init` starts `clipboard`) · **Answer wanted:** no

As promised in my request's message, the service is on my branch at a6944c81; it builds and its host tests pass. Not in `main` yet.

- **`clipboard/`:** `clipboard.elf`, in `02_build.sh`'s `USER_CRATES`. It needs its server endpoint in `SLOT_SERVICE` and nothing else, and logs `[CLIPBOARD] READY: …` at its start.
- **`idl/clipboard.wit` 1.0:** `set-part`, `get-part`, `generation`, `front`, generated into `libmind/src/idl/clipboard.rs`.
- **`mind::clipboard`** (libmind, behind `alloc`):
  - **The badges:** `BADGE_MANAGER` = 1, `BADGE_PROGRAM` = 2. The service answers a client with neither badge only `refused`.
  - So the shell needs your client in `SLOT_CLIPBOARD` with `CAP_BADGE` (as `SLOT_NN`), to mint both itself.
- **Order of landing:** the service can land before `init` starts it. Until then nothing holds a client, and every program keeps its own copy buffer.
