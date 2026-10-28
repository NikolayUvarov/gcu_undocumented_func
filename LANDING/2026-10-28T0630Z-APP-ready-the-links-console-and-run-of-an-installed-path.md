# Ready: the link's lines as the console's in the link-console mode, and `run` of an installed application's path

**From:** APP · **To:** LANDING · **Written:** 2026-10-28 06:30 UTC

**Ready:** `claude/wizardly-franklin-kec1a9` c90fa3129713cd75a1a6095b0811ab425e2b260c

## What it closes or advances

- **Closes [800-APP-0108](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/claude/wizardly-franklin-kec1a9/issues-done/800-APP-0108-the-links-lines-as-the-consoles.done)** (LNK's request, for 800-LNK-0019). While `init` reports `link-console` (`init.wit` 1.8), a line of the link's client:
  - runs a script by its file, its questions answered yes and each answer logged;
  - runs the network policy's changes at once;
  - never runs `mode`, from a script the link started either.

  `mode` and the start notice show `LINK CONSOLE: ON`. Code: `shell/src/clients.rs`, `main.rs`, `msh.rs`, `modectl.rs`.
- **Closes [750-APP-0109](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/claude/wizardly-franklin-kec1a9/issues-done/750-APP-0109-run-takes-an-installed-applications-path.done)** (KRN's request, for 750-KRN-0169). `run` takes a name with `/` up to the loader's 63 bytes; a bare name keeps 16.
- **Advances 351-APP-0029.** The shell's `update` was written and is reverted in this tree (39432555), because the updater refuses calls that lend a buffer. That is UPD's to change, requested in `requests-UPD.md` (`main` dfb74bf0). The issue records it, and the branch's net change has no `update` command.
- **Tests:**
  - `tests/shell_host.rs`: `Client::LinkConsole`;
  - the `modes` suite's reboot step checks `LINK CONSOLE: ON`;
  - `installed_apps_check` runs a package `memtest` by paths past 16 bytes and has the shell refuse a 64-byte path and a 17-byte name.
- **Docs:** `docs/tools/README.md` and `README_RU.md`. The rest is Markdown in `issues/`.

## What was tested, and where (on c90fa312, `main` cd7ad4a8 merged in)

- **Host tests:** `tests/shell_host.rs`, 19 passed.
- **x86, in the tools track's container** (QEMU, TCG, 4 CPUs, OVMF), all passed:
  - `--suites modes,link`;
  - `--suites shell,tools,keys`;
  - `--suites bench`.
- **aarch64, on i9 through Effector** (Ubuntu 24.04 under WSL2, QEMU 8.2.2, TCG), all passed:
  - build (aarch64);
  - "aarch64: programs, shell and four CPUs", 513 s;
  - "aarch64: files, network and TLS", 3065 s.
- **One failure on the way, not this branch's.** On the tree before `main` came in (bf895e4d), "aarch64: programs, shell and four CPUs" failed once on i9, at `diag/quick.msh` step 16.
  - A rerun of `--suites bench` three times there failed once with `check`'s `('the RTC advances', ('✗', '3 s in 1.21 s'))` and passed twice. The whole group then passed.
  - It reads as a race in `bench/src/bin/check.rs`, where the first RTC read comes before `t0`. It was reported to KRN in `KRN/2026-10-28T0504Z-APP-checks-rtc-advances-races-with-its-first-read.md` (e995c007). This branch does not touch `check` or `rtc`.
