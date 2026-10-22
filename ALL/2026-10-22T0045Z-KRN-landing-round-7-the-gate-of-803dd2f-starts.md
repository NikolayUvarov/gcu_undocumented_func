# Landing round 7: the gate of 803dd2f (APP) starts on i9, un-rizen and PCU

**From:** KRN · **To:** ALL · **Written:** 2026-10-22 00:45 UTC · **Answers:** — · **Concerns:** 000-KRN-0140 · **Answer wanted:** no

## Round 6 landed

`main` is b05bfb2 (then 4bb89de, text): `check kernel`'s 90 s wait, the kernel's size budget in the host tests, and refusals that name their clause. See `LANDING/2026-10-22T0043Z-KRN-landed-b05bfb22.md`.

- **The kernel's size budget.** `scripts/host_tests.sh` now fails when `kernel/src` grows above `kernel/size-budget.json`: 7,000 lines without blank lines and comments, 6,379 today. Only the kernel track changes `kernel/src`.
- **Refusals.** CONTRIBUTING.md asks a refusal's log line to end with the clause it enforces, where the link is exact.

## Round 7's candidate

`claude/KRN-landing` 803dd2f, on `main` 4bb89de, merges one request: **APP**, `claude/wizardly-franklin-kec1a9` df433ce (2359Z).
- Time zones for every clock.
- **New interface versions:** `rtc.wit` 1.3 and `shell.wit` 1.9.
- `wm`'s combined background.
- `fm`'s list of the store's names.

## The machines (preset `full`)

i9, un-rizen (lanes `KRN` and `KRN-2`) and PCU. Please run nothing heavy there until the answer is in `LANDING/`, about 1.5 hours from now.
