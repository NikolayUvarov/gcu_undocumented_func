# 213-APP-0075 — The shell logs its first frame, and a slow step before it

**Type:** tools (`shell`), diagnostics · **Owner:** tools track (`APP`) · **Priority:** P1 · **Status:** open: done and checked in QEMU; a real machine's boot log left · **Blocked by:** — · **Main task:** [213](213-amd-ryzen-pc.md) · **Roadmap:** track H · **Constitution:** MC-12.1, MC-12.3

**Recorded by:** the kernel track (KRN), 2026-10-13, in `requests-APP.md`, for [213-KRN-0098](213-KRN-0098-black-screen-when-the-shell-takes-it.md). Numbered by the tools track on 2026-10-14.

## Problem

On the maintainer's Ryzen machine the screen went black when the shell took it. The compositor now logs the screen it shows and how many of its pixels are lit, but the system log still cannot tell a shell that never drew from one that waited on a call. The kernel track asked for:

- one line in the system log when the shell has rendered its first frame, for example `[SHELL] FIRST FRAME 2560X1440 AT 7.9 S`;
- one if a call made before it takes over 5 s, naming the service.

## Plan

- **`shell/src/start.rs`:** times the steps from the shell's start to its first frame:
  - a step that took longer than 5 s gives `[SHELL] <STEP> TOOK <N> MS BEFORE THE FIRST FRAME`;
  - the first frame gives `[SHELL] FIRST FRAME <W>X<H> AT <S.D> S`;
  - after the first frame, nothing.
- **`shell/src/main.rs`** names each step and what it calls:
  - the screen and the serial line, the focus, the consoles' keys (the kernel);
  - the boot's log and new log lines (`logd`);
  - the consoles' programs and notices (the kernel);
  - the serial line;
  - the keys, and the commands typed before the first frame;
  - drawing the first frame.
- **The lines go to the system log** through `println!`, which reaches `logd` with the shell as their source (`dmesg -s shell`).
- **A shell with no screen** logs `[SHELL] NO SCREEN TO DRAW ON: THE SERIAL LINE ONLY` and times nothing.
- **`shell/src/console.rs`:** `render` says whether a frame went to the screen.
- **What it cannot say.** A step that never returns leaves no line of its own. The missing `FIRST FRAME` line, with the compositor's `SCREEN IN FRONT #1 … 0 … PIXELS NOT BLACK`, then says the shell never drew. COM1 was such a step (213-KRN-0098, fixed by the kernel track).

## Acceptance criteria

- **Host test** (`tests/shell_host.rs`):
  - a step of exactly 5 s is not named, and one over 5 s is;
  - each step is timed from the one before;
  - the first frame's line comes once, and nothing comes after it;
  - without a screen, nothing is timed.
- **The `display` suite** (x86, on VGA std, virtio-vga and ramfb; the aarch64 groups run no `display` suite): `dmesg -s shell` has `[SHELL] FIRST FRAME …`. A slow step is not a failure there: a loaded host may make one.
- **A real machine:** the line is in the boot log (`log:bootNNNN.log`).

## Related

[213-KRN-0098](213-KRN-0098-black-screen-when-the-shell-takes-it.md) (the black screen, the compositor's lines), [213](213-amd-ryzen-pc.md) (the Ryzen PC).
