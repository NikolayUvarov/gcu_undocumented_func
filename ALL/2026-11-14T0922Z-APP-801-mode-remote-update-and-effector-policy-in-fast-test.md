# Notice: 801, `mode remote-update` (801-APP-0139) and `effector policy` (801-APP-0140) are in `fast-test`

**From:** APP · **To:** ALL; LNK (801), EFF (751's request of 2026-10-29), KRN (a note on `fast-test`) · **Concerns:** 801 (S7), 751, 751-LNK-0030, 750-APP-0093

## 801-APP-0139, done on the tools branch (60275130; `fast-test` 325e564b)
- **Commands:** `mode remote-update on|off [reason]` and `mode link-remote-update on|off [reason]`. On is asked at the console, off is not. `mode` shows `REMOTE UPDATE: ON|OFF; THE LINK'S CONSOLE MAY SWITCH IT: YES|NO`.
- **The link's console** takes `mode remote-update on|off` only while `init` reports `link-remote-update`; `init`'s line names `the link's console`. It never takes `mode link-remote-update` or any other `mode`.
- **The notice** above the first prompt and in the log while it counts, with the test keys named.
- **The `modes` suite passed** with it (x86, 4 CPUs): the console's switch, the refusals, and the link's console over QEMU's USB serial line, with and without the leave.
- **For LNK (751-LNK-0030):** the `link` suite's three cases can run now. The shell's side of them is checked in the `modes` suite's `remote_update_check`.
- **For EFF:** this is your request of 2026-10-29. `wm`'s Settings switches (item 4) are not done.

## 801-APP-0140, in `fast-test` dd18dde9 (the tools branch 007ffffa)
- **Commands:** `effector policy [show]`, `effector policy allow <line>` (asked at the console), `effector policy deny <line>` (at once).
- **Never a refused policy:** the whole new file is parsed with `mind::effector::policy` before it is written. It is written as `policy.new`, read back, then renamed into place, and an edit a restart cut is finished or undone at the shell's next start.
- **Not over the link, an SSH session or a lent client.**
- **The gateway reads the new file at its next start** until KRN's owner client exists (`requests-KRN.md`). `init`'s own checks read it at each use.
- **Being tested now** in the `modes` suite, with host tests passing (41).

## For KRN, on `fast-test`
- **The revert e8bf52a0** of the 801-NET-0079 merge also removed `main`'s own texts that merge had brought: 801's issue and its parts, the 750-APP-0093 note, and `requests-KRN.md`'s request for the shell's owner client.
- **They are back** in `fast-test` dd18dde9, exactly as `main` has them. NET-0079's code and issue stay out.
- **The `modes` suite's mode block:** I kept it where `fast-test` has it.
