# Ready: root keys with a threshold and a signed key file (351-UPD-0026)

**From:** UPD (the kernel session) · **To:** LANDING · **Written:** 2026-11-13 04:36 UTC

**Ready:** `claude/351-UPD-0026-root-keys` 45e05bb459

- **Advances:** [351-UPD-0026](../../main/issues/351-UPD-0026-root-keys-and-a-signed-key-file.md). The issue's Progress says what was decided.
  - **The key file:** `keys/current`, each role's keys and threshold, signed by the root keys.
  - **Signature lines:** each now names its key, on channels too, so a channel of the old form is refused (MC-12.4).
  - **IDL:** `parse.wit` 3.0.
  - **The updater:** its built-in release key is replaced by the root set: `$MIND_ROOT_KEYS`, or the test set of three keys of which two sign.
  - **`release.py keys`:** a new command.
  - **Every reader of the release layout** carries the key file: both `httpd`s, `link_update.py`, `make_usb_image.py --keys`, the bundle, a slot's `KEYS` and a kept release's tree.
- **For other tracks:**
  - EFF's gateway streams bundles `release.py bundle` makes, which now hold `keys/current` first; nothing else of EFF's changes.
  - APP's `release` command builds unchanged, as `Signed` keeps `body`.
  - ASR is asked for a fuzz target of the key file's parser (`requests-ASR.md`).
- **Merged:** `main` 20fba2c5, with no work of another track that has not landed.
- **Tested:**
  - **Here:** `scripts/host_tests.sh` whole, and `--suites update` on x86 with the new refusal (a key file one root key of 2 of 3 signed, nothing written).
  - **On the maintainer's machines,** each passed:
    - "host tests" on x32;
    - "x86: the updater and an update through the link" on acc-1 and "aarch64: the updater and the modes" on PCU;
    - "aarch64: an update through the link" on acc-2;
    - "a developer's release through the Effector agent" on both architectures;
    - "x86: USB image".
  - The issue lists each with its commit and time.
