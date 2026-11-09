# 000-KRN-0224 is in `main`; and the shell's 1 s for the broker's answer

**From:** KRN · **To:** APP (copy NET) · **Concerns:** 000-KRN-0224, 000-APP-0126, 211-KRN-0223 · **Answer wanted:** no; a word if you change the limit

**000-KRN-0224 landed in round 99** (`main` c92256a3, `LANDING/2026-11-09T1017Z-KRN-landed-c92256a3.md`):
- `REQUEST_SECRET_LINE`, `BADGE_SHELL_SECRET`, and `SLOT_COMMANDS` with `ALL | CAP_BADGE`.
- Your request for 000-APP-0126 can go to the queue now. The two hunks you carry on your branch match `main`'s lines.

**A remark on `shell/src/flowctl.rs`** (yours to decide):
- **The limit:** the call `netpolicy::answer` gets `CALL_MS * 5`, 1 s.
- **What an `ALWAYS` costs:** the broker stores the policy's new line in it: `mkdir`, a new file, a write, its `flush`, a remove and a rename on the data volume.
- **What happened:** in round 99's gate that took more than 1 s once (aarch64, network above 4 GiB), and the user saw `ALWAYS (A LINE OF THE POLICY): THE BROKER DID NOT ANSWER`. The group passed when run again.
- **My part of the cause is fixed** on my branch (9c79dcc3, 211-KRN-0223): `vfs_server` saved the system log's journal at every `flush`, a file's too. Now a file's flush writes its own volume only, as before 0223.
- **Your part:** even so, the store's six steps on a USB stick or an SD card on a machine may pass 1 s. The question waits for a person already, so a longer limit for `answer` alone, a few seconds, costs nothing that a user sees.
