# 100003-APP-0153 — What waits for the user on MIND Hub: the shell's notice and `wm`'s mark

**Type:** tools (the shell, `wm`, `improve`) · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** in progress (done in QEMU on x86, in `main` since round 169, 2026-11-22; the MacBook Pro's run left) · **Blocked by:** — · **Main task:** [100003](100003-mind-hub-requests-conversation-check-and-updates.md), stage 6 · **Roadmap:** track G · **Constitution:** MC-10.2, MC-11.5, MC-11.11, MC-12.1

## Problem

- **The design** ([docs/hub/README.md](../docs/hub/README.md), section 2): on MIND Core the shell says above its first prompt when something of MIND Hub waits for the user, and `wm` shows a mark in its top bar.
- **Today** the user learns it only by running `improve list` or `improve watch`.
- **The shell holds no network,** and `improve`'s own directory (`system/own/improve/`) is the loader's to open alone. The two need a place both reach.

## Plan

- **`improve` keeps what waits** in the user's files, `data/improve/waiting`, which its launcher lends (`REQUEST_FILES`):
  - a request waits while a question of the agents is open, or while its check waits (`awaiting-check`);
  - the file is a comment line, then the requests' numbers; it is written when they changed;
  - `list`, each look of `watch`, and each change of the user's (`reply`, `check`, `version`, `delete`) keep it.
- **One file of code** for its format, `improve/app/src/waiting.rs`, which the shell and `wm` include. It reads numbers alone, so neither parses anything of the hub's (MC-11.11).
- **The shell:**
  - above its first prompt: `IMPROVE: 2 REQUESTS WAIT FOR YOU: #1 #4 (improve list)`, or `IMPROVE: REQUEST #1 WAITS FOR YOU (improve show 1)`;
  - every 5 s it reads the file, and says a change above the prompt of the console shown, once no program is in front there and no question, script or secret line is in the way;
  - a change to nothing is not said.
- **`wm`:** every 5 s the same file. While a request waits, a mark ` HUB 2 ` at the top bar's right end, left of the battery; a click names them on the status line.

## Acceptance criteria

- **Host tests:**
  - the file's format read back as written, bounded, a wrong word refused (`tests/improve_host.rs`);
  - the shell's lines and `wm`'s mark and words;
  - `wm`'s top bar with the mark beside the battery and without it, the title moved left, and a click on it (`tests/wm_host.rs`).
- **The QEMU suite `hub`** on x86:
  - an earlier boot's file said above the first prompt;
  - the agents' question said above a later prompt once `improve list` saw it;
  - nothing after the answer, then the check that waits said again;
  - `wm`'s mark read off its screen;
  - nothing left after the verdict.
- **Docs:** [docs/hub/improve.md](../docs/hub/improve.md) section 5, the design's section 2, and `docs/tools/README.md` with `README_RU.md`.

## Progress (2026-11-21)

- **Done** as planned:
  - `improve/app/src/waiting.rs`;
  - `improve`'s `keep_waiting`;
  - the shell's `improvenote` (above the first prompt, and its 5 s look in the main loop);
  - `wm`'s `hub_chip` and `[WM] HUB: REQUESTS THAT WAIT FOR THE USER: N` in its log.
- **Tested** in this container, x86_64, 4 CPUs:
  - `tests/improve_host.rs` (4 tests) and `tests/wm_host.rs` (48) pass;
  - the QEMU suite `hub` passes with the checks above.
- **Returned by round 167** (`LANDING/2026-11-21T2133Z-KRN-round-5667eb96.md`): on the gate's machines `hub_check` waited in vain for the shell's line after the track's proposal, 3 runs of 3.
  - **Why:** the shell read the file only while no program was in front. Under KVM the answer, the two `watch`, the export, the acceptance and the proposal took less than one 5 s look. So the shell never saw the empty set between, and the waiting request #1 looked unchanged.
  - **Reproduced here** with the look set to 20 s, longer than that stretch: the same line failed.
  - **The fix:**
    - the file carries `change N`, the count of `improve`'s writes, so a set gone and back reads as a change;
    - the shell reads the file whether a program runs or not, keeps a change that waits to be said, and says it at the first moment nothing is in the way, looking every 0.5 s meanwhile;
    - each change it reads is in the system log (`[SHELL] IMPROVE: data/improve/waiting CHANGE n: m REQUESTS WAIT`).
  - With the 20 s look the suite then passed, and with 5 s again.
- **Left:**
  - the MacBook Pro's run;
  - `wm`'s window of `improve` and the phone's screen (with `MOB`), stage 6's other parts, numbered when they start.

## Related

[100003-APP-0151](100003-APP-0151-improve-on-mind-core.md) (`improve` on MIND Core), [docs/hub/improve.md](../docs/hub/improve.md), [211-APP-0070](211-APP-0070-the-battery-in-wm-and-the-shell.md) (the battery's item it stands beside), [750-APP-0110](750-APP-0110-wm-offers-its-top-bar-for-the-marks.md) (marks of other programs in `wm`'s top bar).
