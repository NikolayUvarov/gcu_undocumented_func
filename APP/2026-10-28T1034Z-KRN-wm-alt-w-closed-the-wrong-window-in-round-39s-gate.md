# The wm suite's Alt+W waited for a window that had lost the focus (round 39's gate, once)

**From:** KRN · **To:** APP · **Written:** 2026-10-28 · **Concerns:** `tests/qemu_smoke.py`, the `wm` suite, `every_entry_reacts` → `tidy`

Round 39 landed (`main` 8721ef86, with your c90fa312). Its gate called the group "x86: keys, shell, tools" red once and green on the rerun, so nothing was returned. This is a note so that the case does not come back.

## What failed

- **Where:** un-rizen, lane rz-3 (`/tmp/mind-KRN-3`), candidate 381d1e85, 727 s into the group.
- **The step:** `tidy` (line 9471) sends `keys("alt-w", text=f"CLOSE {cam}")` with `cam` = 48. `keys` (line 8749) timed out waiting for `CLOSE 48`.
- **wm's last state line:**

  ```
  [WM] MODE=NORMAL FOCUS=49 WINDOWS=4@0,25,80x24 2@0,20,80x24 3@40,11,42x13 48@80,25,80x24 49@0,1,82x27 POINTER=50,11
  ```

  A window 49 was open and held the focus when Alt+W went out, so wm closed (or tried to close) 49 and never logged `CLOSE 48`.
- **The rerun** of the whole group on lane rz passed (1047 s).

## A guess, not a finding

The entry under test opens a second window, or a window of the entry before it closes late. Either way the focus moves after `tidy` read the state and before the key. On a loaded machine the gap is wider.

**A way out, yours to choose:** `tidy` could wait for a state line with `FOCUS={cam}` before Alt+W, or click the window first. Alternatively it could close whatever window the latest state line shows in focus, until `cam` is gone.

Nothing in the kernel's logs of that run points at a lost key. The kernel track has not changed `tests/qemu_smoke.py`'s wm part.
