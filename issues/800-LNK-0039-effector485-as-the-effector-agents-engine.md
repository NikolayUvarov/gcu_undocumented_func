# 800-LNK-0039 — `effector485` as the engine of the Effector agent's link mode

**Type:** tool · **Owner:** `LNK` · **Priority:** P1 · **Status:** in progress (2026-11-09) · **Blocked by:** — · **Main task:** [800](800-mind-link.md), for [100000](100000-a-phone-the-pinephone-first.md) · **Constitution:** MC-2.3, MC-7.5, MC-12.3, MC-12.4

Numbered from the kernel session's request of 2026-11-08 in `requests-LNK.md`, for [docs/remote-serial.md](../docs/remote-serial.md) section 5.1 (version 0.1, a proposal).

## Problem

[docs/remote-serial.md](../docs/remote-serial.md) proposes the Effector agent on a Linux or Windows host:
- runs `effector485 daemon` for each port in link mode, as a supervised child on the loopback;
- calls its API, so that it carries no second implementation of the protocol;
- offers each machine's channel 1 as a console session (section 4.3).

**What `effector485` lacks for that:**
- a console stream;
- the bytes outside frames;
- a mode for being started by another program;
- an API version the agent can depend on.

## Plan

1. **A versioned API:** `GET /api/v1/version` with:
   - the API's version and the program's;
   - the operation kinds and their fields, in machine-readable form;
   - the list of changes.

   A change of fields gets a new version (MC-12.4), and `docs/link/effector485.md` lists them.
2. **A supervised mode, `daemon … --supervised`:**
   - refused unless `--listen` is a loopback address and `--key-file` is given;
   - no page: `/` answers 404;
   - it ends when its standard input closes, which happens when its parent ends, on Linux and Windows alike.
3. **The console stream,** for each machine:
   - **Output:** channel 1's output, read from the machine between operations, goes into a ring of 1 MiB with absolute byte offsets. `GET /api/v1/console?agent=…&from=<offset>` answers the bytes from there, each piece with its time, and a `gap` when the bytes asked for were dropped.
   - **Input:** `POST /api/v1/console/input` takes bytes or text with a sequence number. A number already written is not written again (MC-7.5).
   - **Live:** the page and `/api/v1/stream` show the output as it comes.
4. **Bytes outside frames** (a boot's lines before `link` answers, a console that is not the link) go into that ring as console text, with their offsets.
5. **The rule of 2026-10-30** in `docs/link/effector485.md`, with the maintainer's addition of 2026-11-08:
   - local, and reached through Effector's host agent under that host owner's policy;
   - sessions never reach the daemon's port directly.
6. **Tests:** `tools/mindlink/tests/effector485.rs` against `mindlink pty serve`:
   - the version and the kinds;
   - `--supervised` refusing a wide address and ending with its parent;
   - the console's offsets, a gap, an input sent twice and written once;
   - boot lines before the first frame read as console text.

## Acceptance criteria

The tests of plan step 6 pass on Linux. `--supervised` ends with its parent on Windows too, checked on the maintainer's PC.

## Progress (2026-11-09)

**Plan steps 1 to 5 made** on `claude/LNK-link`; the page (step 3's last point) is left:
- **The version:** `GET /api/v1/version`, API 1.1, with the kinds, their required fields and whether each only reads, and the changes since 1.0.
- **`--supervised`:** refused off the loopback and without a key; no page; it ends when its standard input closes.
- **The console** (`console.rs`):
  - a ring of 1 MiB a machine with absolute offsets, a `gap`, and each piece's time;
  - channel 1 read every 250 ms between operations while the machine offers it;
  - input numbered, a number taken already not written again, and a lease's holder's alone.
- **The bytes outside frames:** each run between zeros that is no frame, a run longer than any frame, and one the line left idle. They are read between exchanges as well, and go to the machine's console, or to `line` on a cable of several.
- **A zero before each frame** (`linkproto` `frame::encode`, both ends): it ends a console's line or noise that came before, which spoiled the frame after it. [protocol.md](../docs/link/protocol.md) already let a sender do so, and every receiver takes it as nothing.
- **`mindlink serve --banner`** writes a text outside frames after the first answer, for the test.

**Tested on Linux** (`cargo test` in `tools/mindlink` and `linkproto`):
- `the_console_the_version_and_a_supervised_daemon`: the boot's lines before the console's output, offsets and pieces, an input sent twice written once, base64 input, `--supervised` refused twice, then its page, key and end;
- the other four effector485 tests, `bus.rs`, `mindlink.rs` and `linkproto`'s 24.

`tools/mindlink/build.sh` builds both for Windows; `--supervised` ending with its parent there is not checked yet.

## Related

[100000-LNK-0038](100000-LNK-0038-link-over-the-boards-console-uart.md), [800-LNK-0019](800-LNK-0019-the-link-as-the-owners-console.md), [docs/link/effector485.md](../docs/link/effector485.md), [docs/remote-serial.md](../docs/remote-serial.md), [requests-EFF.md](requests-EFF.md).
