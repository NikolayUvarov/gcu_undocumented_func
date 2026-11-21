# KRN: ready — a release's downloads made from one built tree, listed and signed (352-UPD-0050)

**Ready:** `claude/352-UPD-0050-release-artifacts` 8586e2f53e

**For round 164.** It holds `claude/352-UPD-0047-installer-iso` 8237936bb2, which round 163 is gating; `main` cb5dc7f6e0 merged in. It does not need 352-KRN-0268, and merges cleanly after it.

**What it lands (352-UPD-0050, advanced):**
- **`scripts/release.py sums DIR [--keys FILE]`:** `SHA256SUMS` in sha256sum's form, `SHA256SUMS.sig` with the release keys' signature lines, and `KEYS`, the key file;
- **`verify-sums DIR`:** the list's signature checked against the root set and every listed file's hash; a changed byte, a missing file or an unlisted name is refused;
- **`artifacts VERSION OUT [--arch] [--channel] [--date]`:** from one built tree, each architecture's ISO (`make_iso.py`), its USB image (`make_usb_image.py`) and its update bundle, then `RELEASE.json` and the signed list;
- **`tests/release_test.py`:** 18 tests, 2 of them new.
- **Docs:** docs/update/publishing.md 0.9 (EN and RU), and the issue's progress.
- **No IDL or ABI change.** Nothing of it goes into an image.

**Tested** in this session's container:
- `tests/release_test.py` 18 of 18, `tests/make_iso_test.py` 4 of 4;
- a real `artifacts` run on an x86 build, signed, then `verify-sums`: VERIFIED;
- two runs with the same date: the same ISO, bundle and `RELEASE.json`. The USB image differs, as the issue says (its FAT's dates); that is left for step 4.

The host tests run in the gate.
