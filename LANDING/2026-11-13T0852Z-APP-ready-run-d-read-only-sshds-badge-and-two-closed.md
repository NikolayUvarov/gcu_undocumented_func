# Ready: `run -d` of a program that only reads, `sshd`'s badge from `mind::process`; 112-APP-0136 and 312-APP-0134 closed

**Ready:** `claude/wizardly-franklin-kec1a9` 0ba930039e6763fad1e91163bc8287e024dfecec

It contains `main` 7c1c68f563 (round 128).

- **Closes** 112-APP-0136 (the shell takes an SSH session's lines) and 312-APP-0134 (`run -d`), both moved to `issues-done/`. Main task 112's criteria are met with it (KRN, 2026-11-13).
- **112-APP-0136:** the shell compares the remote copy's badge with `mind::process::BADGE_SHELL_REMOTE` (112-NET-0072), and its own name for it goes.
- **312-APP-0134:** for a program that asks only for `REQUEST_BLOCKSTORE_READ`, `run -d` asks through the shell's writing client for `domain-client-with(id, BADGE_GET)` (312-STO-0049), so the program holds the get badge's rights alone in the domain; the refusal it gave until then goes. `run_in_domain_check` checks `blocksro` there and that `blocks` started with `-d` writes and reads a name of its own and reaches none of the volume's.
- **000-APP-0032:** `clipboard` no longer logs a line for each program named in front (KRN's remark of 2026-11-13: about a line a command, which twice pushed a check's line out).
- **Tested** (QEMU, 4 CPUs, in this container, on fd6f7bbb with `main` of round 127; round 128's merge brings only UPD's updater and a comment in `init`):
  - x86: `tools` (11, the clipboard check on `main`'s service), `tls` (13, `sshd_check`: the console key's `ps` run, `update` refused, each word logged), `store` (33);
  - aarch64: `store` (38, `run -d` in live domains, `blocksro` reading and refused a put and a publish);
  - host tests: `shell_host` (38), `clipboard_host` (4), `fm_host` (23) after round 128's merge; the shell and the service build.
