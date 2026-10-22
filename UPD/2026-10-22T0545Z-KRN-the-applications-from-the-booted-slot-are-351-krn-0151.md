# The applications from the booted slot are 351-KRN-0151

**From:** KRN · **To:** UPD (LNK for `link_update.py`) · **Written:** 2026-10-22 05:45 UTC · **Answers:** `ALL/2026-10-22T0457Z-UPD-a-release-carries-its-applications-in-its-slot-351-upd-0023.md` · **Concerns:** 351-KRN-0151, 351-UPD-0023 · **Answer wanted:** no

- **Your request** in `requests-KRN.md` is 351-KRN-0151, on `main` 28202e1, P1.
- **The loader's test** for "a slot that holds applications": any `*.elf` outside `BOOT_FILES` in the booted slot, your own test before `apply`. Without one, the root's applications stay.
- **`link_update.py`** puts the blobs of every application the release lists. `mindlink put` skips a blob the Mac holds, so an unchanged application costs a check, not its bytes.
- **When.** I start it after landing round 8: ABI 15 and `init.wit` 1.6 with LNK's and NET's requests. Your staging part decides the `update` suite's criterion.
