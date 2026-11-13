# 000-KRN-0241 — The watchdog suite reads the shell's safe-start notice across a kernel line

**Type:** kernel track (a test of the gate, `tests/qemu_smoke.py`) · **Owner:** `KRN` · **Priority:** P2 · **Status:** in progress (made and run on the machines; the gate left) · **Blocked by:** — · **Main task:** — · **Roadmap:** track A · **Constitution:** MC-12.2

## Problem

**Round 132's gate,** candidate 712739af (`main` 4b33c841 with STO's 312-STO-0051, a change to the `store` and `boot` suites only), "x86: the watchdog" on x32-128 (59 s). `watchdog_suite` failed its first assertion on the shell's notice:
- **What the shell printed:** `SAFE START: THE LAST RUN DID NOT END AS ASKED. LEFT OUT: audio_gw, tts, video_gw (svc start audio_gw, svc start tts, svc start video_gw)`.
- **What reached the serial line:** the kernel's ACPI line inside it, whole: `LEFT OUT: audio_gw, tt` + `MIND CORE KERNEL: ACPI: IN ACPI MODE ALREADY; EVERY GPE AND FIXED EVENT DISABLED, THE PM1 EVENTS LENT` + a newline + `s, video_gw (…)`.
- **What the suite read:** the notice's regular expression, by line, found no `tts` and failed.
- **The rest of that boot:** nothing was wrong with it. The rerun on x32 passed (129 s), and the gate's other groups did not touch the watchdog.

The kernel and the shell share COM1. Other suites already read again a line that a kernel line lands in (000-KRN-0136, the updater's confirmation; the services' lines read with `dmesg`); this one did not.

## Plan

`watchdog_suite` takes every `MIND CORE KERNEL: …` line out of what it read, with its newline, before it looks for the notice. A kernel line is printed whole, so the shell's line is whole again around it. The assertion still prints what was read, untouched.

## Acceptance criteria

- The expression finds the notice in round 132's text (the kernel line inside `tts`) and in a notice with no kernel line in it.
- "x86: the watchdog" passes on the maintainer's machines, then in a gate.

## Progress

**Made (2026-11-13)** on `claude/000-KRN-0241-safe-start-notice-across-a-kernel-line`: `watchdog_suite` reads the notice in what the serial line gave with every `MIND CORE KERNEL: …` line and its newline taken out.
- **The expression** found the notice in round 132's text (the ACPI line inside `tts`) and in a notice with no kernel line in it, run on both strings in this container.
- **Run** at 5085be75 (`main` d35515e0 merged in) on the maintainer's machines (QEMU under TCG): "x86: the watchdog" on x32-128 passed (130 s). That run had no kernel line inside the notice; the expression's case is the one above.
- **Left:** the gate.

## Related

[000-KRN-0136](../issues-done/000-KRN-0136-the-update-suite-waits-for-the-whole-confirmation.done), `watchdog_suite` in `tests/qemu_smoke.py`.
