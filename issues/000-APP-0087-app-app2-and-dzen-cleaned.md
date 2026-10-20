# 000-APP-0087 — `app`, `app2` and `dzen-clock`: libmind's drawing, no `static mut`, the right words on exit

**Type:** tools (demo programs) · **Owner:** tools track (`APP`) · **Priority:** P3 · **Status:** open (planned) · **Blocked by:** — · **Main task:** — · **Roadmap:** track G · **Constitution:** MC-12.3

**Recorded by:** the kernel track (KRN), 2026-10-14, in `requests-APP.md`, from the critique [issues-critica-done/02](../issues-critica-done/02_code_and_comliance_analysis.md), §3. Numbered by the tools track on 2026-10-20, at the maintainer's word: the demo programs are user programs, so the tools track takes them.

## Problem

- **`app`:**
  - `MAIN_SP`, `THREAD_SP`, `WORK_COUNTER` and `THREAD_STACK` are `static mut`;
  - `background_task` never returns but is not `-> !`;
  - it draws with its own `draw_char`, `draw_string` and `usize_to_str`, with no bounds check, straight into `fb_ptr`.
- **`app2`:**
  - it has its own `fill_rect` and `draw_text`;
  - on Esc it logs `RETURNING TO KERNEL` though it returns to the shell;
  - it reads the sprite through `as_mut_slice()` where `as_slice()` does.
- **`dzen-clock`** has the same "returning to kernel" wording.

## Plan

- **`app`:**
  - atomics for the counters and pointers, and an `UnsafeCell` wrapper for the stack;
  - `fn background_task() -> !`;
  - `mind::gfx::Screen` and `mind::util::Decimal` for its drawing;
  - the pixel colours the `normal` suite checks are kept: cyan, and red after Space.
- **`app2`:** `Screen` for its rectangles and text; `as_slice()`.
- **`app2` and `dzen-clock`:** "RETURNING TO THE SHELL", with the suites' checks changed with it.

## Acceptance criteria

- The `normal`, `busy`, `smp` and `isolation` suites pass on x86 and aarch64; `boot` runs `app`.
- `grep -rn "static mut" app app2` and `grep -rn "RETURNING TO KERNEL"` find nothing.

## Related

[issues-critica-done/02](../issues-critica-done/02_code_and_comliance_analysis.md), §3.
