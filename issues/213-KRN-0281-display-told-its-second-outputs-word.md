# 213-KRN-0281 — `display` told its second output's word from `data/displays.txt`

**Type:** `init`, `libmind` (no ABI change) · **Owner:** `KRN` (the kernel session) · **Priority:** P1 · **Status:** in progress (in `main` since round 177, bf0e9db15a, 2026-11-26, with DRV's 213-DRV-0099; the Ryzen run left) · **Blocked by:** — · **Main task:** [213](213-amd-ryzen-pc.md) · **Constitution:** MC-12.1, MC-12.3

## Problem

DRV's 213-DRV-0099 ("both screens by default") replaces build switches with a setting of the machine (DRV's message of 2026-11-25, `KRN/2026-11-25T2203Z-DRV-…`, ask 1). The setting is the line `output second off|read|light` of `data/displays.txt`. `display` reads it as the word after the device's index in its argument (`24 light`), and `init` passes it, as it passes nnd's line of `data/memory.conf`.

## Plan (made)

- **`libmind`'s `displays`:**
  - `FILE` and `MAX`;
  - `Output` (`off`, `read`, `light`);
  - `second_output(text)`: the first `output second` line, its case aside. The answer is the word, another word, or no line.
  - The shell can read the file with the same code.
- **`init`'s `display` arm:**
  - reads the file at each start and passes `<device> <word>`;
  - with no line, or another word, it passes `off` and says which: `[INIT] display: SECOND OUTPUT light BY data/displays.txt`, `NO \`output second\` LINE …: off` or `… IS NOT off, read OR light: off`.
- **`tests/displays_host.rs`:**
  - the shell's lines alone;
  - each word, its case aside, with a comment after it;
  - a Windows line end;
  - another word, more words, an empty word;
  - the first line counts.
- **The profile:** `bootstrap.md`'s `display` row.

`display` on `main` prints its argument as the device's index. Until DRV's branch lands, the word shows there, and nothing else changes.

## Acceptance criteria

- The host test passes, and `init` builds on both architectures.
- On the Ryzen, in DRV's run: `init`'s line names the word, and `display` takes it, for that machine and build only (MC-12.1).

## Related

[213-DRV-0099](../issues-done/213-DRV-0099-display-both-screens-by-default.done), [213-APP-0158](213-APP-0158-the-second-screens-fit-at-the-console.md) (the shell's `second keep|stretch` in the same file).
