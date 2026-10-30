# 751-UPD-0032 — `install`: a developer's release streamed in by the Effector gateway (`update.wit` 1.3)

**Type:** service interface (`updater`, `idl/update.wit`, `scripts/release.py`, `tests/updater_stub`) · **Owner:** `UPD` (the kernel session) · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Main task:** [751](751-system-updates-at-a-developers-request.md) · **Constitution:** MC-9.2–9.4, MC-12.6, MC-10.3

**Asked by:** EFF for 751-EFF-0023, in `UPD/2026-10-30T0634Z-EFF-751-the-install-calls-names-and-the-bundle-format.md` and `KRN/2026-10-30T0738Z-EFF-751-the-test-keys-decided-and-the-install-shape.md` on `internal-dev-messages`.

## Problem

- **What the server holds.** A developer's release for a machine in active development reaches the Effector server, not the machine's `update.txt` source.
- **What the gateway may not do.** It must install nothing itself: it holds no writable scope the updater reads (751-ASR-0013's row "the gateway installs on its own").
- **What `update.wit` 1.2 has:** `fetch` from the source, then `apply`. Nothing the gateway can hand bytes to.

## Plan

- **`update.wit` 1.3**, from `BADGE_UPDATE_GATEWAY` only, while `init`'s `remote-update` counts (asked at `install-begin` and at `install-commit`):
  - `install-begin(size: u64, sha256: string<64>, command: string<64>) -> result<u32, error>`: a handle. `command` is the gateway's command ID, for the updater's log (MC-10.3). A second begin drops an open one.
  - `install-write(handle: u32, offset: u64, bytes: list<u8, 4096>) -> result<_, error>`.
  - `install-commit(handle: u32) -> result<u64, error>`: checks the bundle's size and SHA-256, then stages it in the other slot as `fetch` stages a source, and returns the version staged.
  - The gateway then calls `apply(tries)` itself, and `apply` keeps its own check of the word.
- **The bundle:** one ZIP with stored entries, holding the tree `scripts/release.py` publishes:
  - `blobs/…`;
  - `releases/<version>/<arch>/MANIFEST` and `MANIFEST.SIG`;
  - the channel file.
- **The checks:** the updater checks a bundle as any source:
  - the channel with the release key;
  - the manifest with the boot key;
  - each file by size and SHA-256;
  - the floor.
- **Where the bytes go.** The bytes go to the updater's own staging in its update zone, not to a file the gateway can write. The updater reads the ZIP there as a source, then removes it.
- **Errors,** 1.2's:
  - `denied`: the word does not count, or another badge;
  - `digest`: the size or SHA-256 differs;
  - `malformed`, `signature`, `expired`, `older`, `write`, `trial`.
- **`scripts/release.py bundle`** writes the ZIP. **`tests/updater_stub`** answers the three calls, so that EFF can test the gateway in the `effector` suite meanwhile.

## Acceptance criteria

- **Host tests:**
  - the bundle's reader on a ZIP from `release.py bundle`;
  - a bundle with one byte changed refused (`digest`);
  - a manifest not signed by the boot key refused (`signature`).
- **QEMU, x86 and aarch64, the `update` suite:**
  - a gateway-badged client streams a bundle, commits it and applies it with `remote-update` on, and the machine boots the new slot on trial;
  - with the word off, `install-begin` and `install-commit` are `denied`, and the updater logs it;
  - another badge is `denied`.
- **`tests/updater_stub`** answers the three calls.

## Progress

**2026-10-30, on `claude/youthful-mendel-mf1soy`:**
- **`update.wit` 1.3:** the three calls. `install-write` takes `bytes<32768>`, not `list<u8, 4096>`: the same `&[u8]` on the client's side, and a piece of the gateway's own `read` (`effector.wit`, 32 KiB) passes through whole. `tests/idl_fuzz_host.rs` decodes it with a scratch buffer.
- **`tests/updater_stub`** answers them: from `BADGE_UPDATE_GATEWAY` only, the word asked at begin and commit, the bundle hashed as it comes and kept nowhere (so a write resumes only at its end or from 0), version 1 answered; `apply` asks init to restart the machine.
- **`scripts/release.py bundle DEST OUT [--channel] [--arch]`**, after `check`: the channel, the manifests and signatures, the blobs they list; stored, fixed dates. `tests/release_test.py` checks its entries, that it holds only what the channel's manifest lists, that the same release makes the same file, and that a layout that does not verify is not bundled.
- **The updater:**
  - the bundle lies in `MIND/<other slot>/INSTALL/BUNDLE.ZIP`: in its zone, a directory that `stage()` keeps and the bootloader does not read;
  - `install-write` at most at the bytes written so far, the rest truncated;
  - `install-commit` checks size and SHA-256 (`digest`), reads the ZIP in place (`updater/src/bundle.rs`), and stages through the same `stage()` as `fetch`, its lines named `INSTALL`;
  - the bundle is removed after a commit, at a new begin and at start.
- **Host tests:** `tests/updater_host.rs` reads a bundle `release.py bundle` wrote, every entry as the server holds it, and refuses a byte past the end record, a count one more, a compressed or encrypted entry, a local name that differs, and Python's deflated, escaping, repeated and absolute names.

- **The QEMU `install` suite** (in `--suites updater`, so in both architectures' updater groups; alone with `--suites install`):
  - `tests/updater_stub` stands in for the gateway in effector_gw's place. It holds the updater's client badged `BADGE_UPDATE_GATEWAY` and hands over the bundles `system/effector_gw/install.txt` lists.
  - Boot 1, `remote-update` off: four `install-begin` and four `install-commit` calls are `denied`, and the updater logs each.
  - Boot 2, on:
    - a bundle with one byte of its first boot file changed: `digest`, the file named;
    - a manifest signed by another key: `signature`;
    - a bundle cut after its first piece: `digest`, not the size begun;
    - release 6's bundle, its second piece written twice: staged in slot B, then the bundle is removed;
    - `apply` (the word asked again) boots slot B on trial, and the trial is confirmed.
  - The two refusals the plan put on the host (`digest`, `signature`) are checked here, where the updater's built-in keys are.
  - **x86: passed** locally in QEMU (about 3 minutes, 2026-10-30).
- **Fixed on the way:** the updater received its requests with no slot for a caller's buffer, so the first buffer call (`install-begin`) was refused `invalid`. It now receives into slot 9, as the other services do; the stand-in too.

**Left:**
- the `install` suite on aarch64;
- "another badge is `denied`" has no client to run it in QEMU: the shell holds the updater's unbadged client for its `update` command (351-APP-0029), which no program uses yet.

## Related

[751](751-system-updates-at-a-developers-request.md), [751-KRN-0185](751-KRN-0185-the-remote-update-permission-in-init.md), [750-UPD-0020](750-UPD-0020-the-gateways-client-of-the-updater.md), 751-EFF-0023, [751-ASR-0013](751-ASR-0013-the-remote-update-path-threat-model-and-refusals.md).
