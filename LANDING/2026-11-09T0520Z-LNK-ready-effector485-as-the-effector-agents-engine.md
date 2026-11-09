# LNK → LANDING: ready, effector485 as the Effector agent's engine (800-LNK-0039)

**Ready:** `claude/LNK-link` 94c3f12e

**On `main`** 6eed1563 (a fast-forward from it).

## What it changes

**Numbers** the kernel session's two requests (text, 7d124380):
- 100000-LNK-0038, `link` over the board's console UART;
- 800-LNK-0039;
- `requests-LNK.md` emptied.

**Advances 800-LNK-0039** (8222ae09, 94c3f12e). It stays open for `--supervised` on Windows.
- **`effector485`:**
  - `GET /api/v1/version`: API 1.1, the kinds and their fields, the changes since 1.0;
  - `daemon --supervised`: loopback and a key only, no page, and it ends when its standard input closes;
  - `GET /api/v1/console` and `POST /api/v1/console/input`:
    - a ring of 1 MiB a machine, with absolute offsets and a `gap`;
    - channel 1 read between operations while the machine offers it;
    - input numbered and written once, and only the lease holder's;
  - the cable's bytes outside frames, a boot's lines, go into the console as text;
  - the page shows the console.
- **`linkproto`:** a zero before each frame, at both ends, which touches MIND Core's `link` too. It ends a console's line or noise that came before the frame, which spoiled it. `protocol.md` already let a sender do so, and every receiver takes an empty run as nothing. `MAX_WIRE` grows by one; the decoder's limit (`MAX_RUN + 1`) is as before.
- **`mindlink serve --banner`,** for the test.
- **Docs:** `docs/link/effector485.md`, `protocol.md` and `mindlink.md`, English and Russian.

## What was tested, where

On this machine:
- `cargo test` in `tools/mindlink`, all pass:
  - effector485's 5, among them the new `the_console_the_version_and_a_supervised_daemon`;
  - `bus.rs`, `mindlink.rs`, the units.
- `cargo test` in `linkproto`: 24 pass.
- `tools/mindlink/build.sh`: Linux and Windows.
- `scripts/ci_local.sh --only host,x86`, all PASS, on 8222ae09's tree (94c3f12e adds only the page and its docs):
  - `build (x86)`;
  - `build (x86 test programs)`;
  - `x86: services, storage, audio`, with the `link` suite (mindlink through QEMU's FT232, 64 KiB each way) and `modes`.
- The whole `scripts/host_tests.sh` with `MIND_SSHD` on 94c3f12e: 193 suites, none failed.

**Not yet on hardware:**
- the MacBook Pro and the DEXP get the frame's leading zero with their next slot update;
- the maintainer's running `effector485.exe` is older, which the wire allows.
