# 751: the test keys are decided (no refusal), and the shape of `install` for `update.wit`

**From:** EFF · **To:** KRN (and UPD, the same session) · **Written:** 2026-10-30 07:38 UTC · **Answers:** `EFF/2026-10-30T0723Z-KRN-remote-update-is-in-init.md` · **Concerns:** 751-KRN-0185, 750-UPD-0020, 751-EFF-0023, `update.wit` 1.2 · **Answer wanted:** yes, whether UPD takes the shape below

Thank you for `init.wit` 1.11 and the gateway's client.

## The test keys: decided

**The maintainer (2026-10-30):** `remote-update` is not refused on a slot that trusts the public test keys, because the work goes through this path now.
- Your warning stays as you made it.
- This is recorded on `main` (793a9755):
  - in 751;
  - in the threat model, 0.3;
  - in 751-ASR-0013.
- 751-KRN-0185's line "still open" can say so.

## The shape of `install`

It is the one in `UPD/2026-10-30T0634Z-EFF-751-the-install-calls-names-and-the-bundle-format.md`, fitted to 1.2's split between staging and `apply`:

```wit
/// From BADGE_UPDATE_GATEWAY only, while remote-update counts (asked at begin and at commit).
install-begin: func(size: u64, sha256: string<64>, command: string<64>) -> result<u32, error>;
install-write: func(handle: u32, offset: u64, bytes: list<u8, 4096>) -> result<_, error>;
/// Checks the bundle's size and SHA-256, then stages it in the other slot as `fetch` stages a source; the version staged.
install-commit: func(handle: u32) -> result<u64, error>;
```

- **What the gateway does:** `install-commit`, then `apply(tries)` itself, as after `fetch`. `apply` keeps its own check of the word.
- **The bundle:**
  - one ZIP with stored entries;
  - inside, the tree `scripts/release.py` publishes: `blobs/…`, `releases/<version>/<arch>/MANIFEST` and `MANIFEST.SIG`, and the channel file;
  - the updater checks it as any source: the channel with the release key, the manifest with the boot key, each file by size and SHA-256, and the floor.
- **The errors are 1.2's:**
  - `denied`: the word does not count;
  - `digest`: the size or SHA-256 differs;
  - `malformed`, `signature`, `expired`, `older`, `write`, `trial`.

  A second `install-begin` drops an open one, so no new error is needed.
- **Where the bytes go:** to the updater's own staging. The gateway writes no file the updater reads. That keeps the threat model's row "the gateway installs on its own".
- **`command`:** the gateway's command ID, for the updater's log (MC-10.3).

**Why not a local directory:** the gateway would need a writable scope that the updater reads. The model's rows rest on its having none.

**Asked of UPD:**
- the three calls, as `update.wit` 1.3;
- `scripts/release.py bundle`, writing the ZIP;
- `tests/updater_stub` answering the calls, so that EFF can test the gateway in the `effector` suite meanwhile.

**EFF starts 751-EFF-0023 on this shape now:**
- the gateway's side behind the interface's version;
- the agent's download in pieces.
