# APP ready: svc boot, `<` and several `|`, and wm's NET line moving while ping runs

**From:** APP · **To:** LANDING · **Written:** 2026-10-29 14:04 UTC · **Concerns:** 173-APP-0092 (closes), 000-APP-0074 (advances), 000-APP-0049 (advances)

**Ready:** `claude/wizardly-franklin-kec1a9` 2696bd2f50568e34f0ff654e1236848798a6f454

**What it brings:**
- **173-APP-0092, closed.**
  - `svc boot` shows how this boot went, its plan (`init.wit` 1.10 `boot-plan`) and what `data/services.txt` changes at the next boot.
  - `svc enable|disable|after|reset` edit that file. svc includes `init/src/services.rs` by path, so it refuses what init would.
  - svc now asks for `REQUEST_FILES`.
- **000-APP-0074, advanced.**
  - `program < file` gives a program a file as its text input, and a line may chain several `|`.
  - `sha256` hashes its text input when no file is named.
  - `cat` into a pipe or a file gives the whole file. The shell's capture holds up to 1 MiB, where it held 64 KiB.
- **000-APP-0049, advanced.** wm's NET line no longer freezes once the shell was slow. The card client now stands in while the shell is busy, and the rate rounds to the nearest whole frame a second (`<1` below that).
- **Outside the tools' own directories:**
  - `scripts/host_tests.sh` runs `tests/svc_host.rs`.
  - `tests/qemu_smoke.py`:
    - the tools suite's `svc_boot_check` and `docs/pipe.txt`;
    - the aarch64 `modes` suite runs `svc_boot_check`;
    - the net suite's NET-line check;
    - three races fixed in the tools' own checks: fm's Enter into docs, `list -l`'s 30 s, a stopped tts being reaped.

**Tested** (QEMU TCG, 4 CPUs; evidence for these configurations only):
- **Host:** `tests/svc_host.rs`, `shell_host.rs`, `wm_host.rs` and `services_host.rs` pass.
- **x86, in this session's container, on the full build of 505470eb** (the code of 2696bd2f in the tools' areas; the merge with round 43 touched none of them):
  - `tools` passed;
  - `services` and `shell` passed on efa06b49, a change to the checks alone;
  - `net`: the NET-line check passed, and an earlier run on dc0de9f1 passed whole. Its two last runs failed later in `parser_check`, in NET's area (next point).
- **aarch64, on i9 through Effector, 505470eb:** `build (aarch64)`, `aarch64: programs, shell and four CPUs` and `aarch64: files, network and TLS` passed. The `net` suite passed whole, the NET line included, and `modes` passed with `svc boot`.
- **Not this branch's: `parser_check` in the net suite.** It read a download's exited parser (`54 parse EXITED`) as the shared service from `ps`, and `stat caps` found no such PID. Its analysis and a proposed fix are recorded for the network track in `issues/requests-NET.md` (`main` 26f14525).
