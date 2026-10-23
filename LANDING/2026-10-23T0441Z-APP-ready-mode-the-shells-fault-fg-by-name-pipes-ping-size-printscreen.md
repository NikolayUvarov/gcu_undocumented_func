# Ready: the shell's `mode`, its fault after a caller gave up, `fg` by name, `|`, `ping -s`/`-p`, PrintScreen

**From:** APP · **To:** LANDING · **Written:** 2026-10-23 04:41 UTC · **Concerns:** 750-APP-0105, 000-APP-0106, 000-APP-0107, 000-APP-0074, 000-APP-0095, 000-APP-0089, 550-APP-0104, 351-APP-0103, 800-APP-0108

**Ready:** `claude/wizardly-franklin-kec1a9` 3a42ad3e50cc943eb1c4547ddf73e4a223352bf6

- **The hash, checked** with `git rev-parse` and `git merge-base --is-ancestor` on `origin/claude/wizardly-franklin-kec1a9`.
- **`main` in it:** bb7262bd, with landing rounds 13, 14 and 15 and the text-only commits since: main tasks 253–263, the track `NRL`. Merged with no conflict at 451cfb28 (round 15's code), then twice for text only.
- **No ABI and no interface changes.** It uses what `main` already has:
  - `init.wit` 1.5 `mode`/`set-mode` and `starts`;
  - 000-KRN-0094's `commit_with_input`;
  - `socket.wit` 2.6 `ping-with`.
- **Beside KRN's request for this round** (`claude/youthful-mendel-mf1soy` c10b15b5):
  - Both change the `modes` suite. The shell's part now prints a PASS line of its own (ccdf5e3b) and leaves the suite's last PASS line as `main` has it.
  - Round 16's candidate da631b5e (KRN's 40f1fa8d) with this request merged on top has no conflict (tried locally at 04:40 UTC).
  - KRN's new steps run in VMs of their own, after the shell's steps.

## What it brings

- **750-APP-0105, closed: `mode`.**
  - It shows the modes, and turns one on after the operator agrees at the console, off at once, through the shell's `BADGE_MODE`.
  - It is refused to every lent client and to the link.
  - The shell names the mode above its first prompt.
- **000-APP-0106, advanced: the MacBook Pro's shell fault** (LNK's, EFF's and ASR's reports).
  - The shell drops a call whose caller gave up, rather than write its revoked buffer.
  - A restarted shell gives the running `wm` its screen.
  - `autorun.msh` runs once a boot. ASR's `giveup` suite is left.
- **000-APP-0107, closed: `fg <name>`.** Several programs that answer are listed by PID.
- **000-APP-0074, advanced: `|`.** `ps | grep wm`: what the command before prints is the program's text input. `grep` reads it without a path. `<` is left.
- **000-APP-0095, closed: `ping -s` and `-p`.** They go over NET's `ping-with`, and each reply's size and data are checked.
- **000-APP-0089, advanced: PrintScreen** takes a screenshot (x86). The Mac's run is left.
- **550-APP-0104, closed:** the `wifi` guide names the station's own join.
- **351-APP-0103:** closed, its code landed in round 13.
- **800-APP-0108:** recorded, LNK's `link-console` request; no code yet.
- **For the track `NRL`, at the maintainer's request:** its reference prototype `nrl/reference/qwen-spline-lab-v0.1.0.zip` (MIT, SHA-256 5e27aab0…, kept unchanged, built and run by nothing, in no image) and its row in `THIRD_PARTY.md` (3a42ad3e). Its issue 900 and `TRACKS.md` are in `main` already.
- **Two suites' fixes:**
  - the `wm` suite reads close all's own line after the shell's restart (5c556d68);
  - the `shell` suite's scrollback check helps until the banner is off the screen. `help` grew past 400 lines at aarch64's 100 columns (32793d05).

## What was run

QEMU only (MC-12.1). Locally means in this session's container; i7-930 means DESKTOP-i7-930-wsl-u24 through Effector. i9 is offline.

- **On 451cfb28** (`main` 16a52eea, round 15's `init`), locally: the x86 builds, "services, storage, audio", and "keys, shell, tools". The keys group ran again after a container restart, on 40f709dd, which differs from 451cfb28 in text only.
- **Since 451cfb28** only text, the archive and `THIRD_PARTY.md` changed; no suite reads them.
- **On ab223337** (`main` be0e2ec5):
  - x86, locally: the builds, "boot, display, network, TLS, shell, memory, clock", "services, storage, audio", "keys, shell, tools", "files and block writes";
  - aarch64 on i7-930: the build, "programs, shell and four CPUs", and the suites `vfs`, `store`, `storefaults`, `net`, `tls`;
  - aarch64 `effector` and `updater` on i7-930;
  - aarch64 `linkupdate` and `modes` locally, because i7-930's operation reached its hour inside `linkupdate`. Since round 14 its link transfer is 100 files, which took 2552 s there and 1081 s locally.
- **On 11b6ad01** (`main` 2f85e06a, ABI 17):
  - the host tests, locally;
  - the same x86 groups, locally;
  - aarch64 "programs, shell and four CPUs", and the suites `net`, `tls`, `effector`, `updater`, `linkupdate`, `modes`, on i7-930.

**Scope:**
- Directories: `shell`, `search`.
- Tests: `tests/qemu_smoke.py` (the `modes`, `shell`, `wm`, tools and search, `net` and `vfs` checks), `tests/shell_host.rs`, `tests/search_host.rs`.
- Text: `docs/tools`, `issues`, `issues-done`.
- Data: `nrl/reference/` and `THIRD_PARTY.md` (for `NRL`).
