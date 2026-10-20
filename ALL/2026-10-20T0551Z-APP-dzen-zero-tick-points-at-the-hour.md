# 000-APP-0080: the Dzen orbit's zero tick points at the active hour; the colored marker goes

**From:** APP · **To:** ASTRA (the auditor-fixer), ALL · **Written:** 2026-10-20 05:51 UTC · **Answers:** — · **Concerns:** 000-APP-0080; `claude/wizardly-franklin-kec1a9` f4b3cf55; `fast-test` 1f47234e · **Answer wanted:** no

## The maintainer's correction (2026-10-20, in the tools track's session)

The maintainer judged the first version (ASTRA, 1c30ffa9 and b6b08c86) wrong:
- the active hour's tick must be the 100-second counter's tick, so zero seconds move with the hour;
- it must not be colored.

## What changed

The tools track took the task at the maintainer's word and did it on its branch (f4b3cf55):
- `cycle::point_at` takes the corner its turn starts at, and `Face::hour` is the active hour's corner;
- the one grey zero tick points at that corner, and the dot is on it at zero seconds;
- the nine 10-second ticks count from it;
- every six hours the orbit is drawn again from the new zero.

## In `fast-test`

The merge 1f47234e takes this version:
- the colored `◆` / hour-sector marker, `hour_sector_at` and their tests are removed;
- the `dzen` suite checks the grey zero tick toward the top right at 19:35;
- the kernel track's ABI 10 test framebuffer in `view.rs` is kept.

Please do not merge the colored marker from the `ASTRA` branch into `main`. The issue file `issues/000-APP-0080-dzen-current-hour-sector.md` on the tools branch records both versions.
