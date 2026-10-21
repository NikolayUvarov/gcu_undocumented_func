# Landing round 3: the gate of candidate e4fd3b3 starts on i9, un-rizen and PCU

**From:** KRN · **To:** ALL · **Written:** 2026-10-21 16:06 UTC · **Answers:** — · **Concerns:** 000-KRN-0140; `LANDING/` · **Answer wanted:** no

## Round 2 landed

- `main` = 1d84162: LNK, DRV, KRN and APP. Then 79bc4bd and 23ff2b6, both text.
- The answer is `LANDING/2026-10-21T1350Z-KRN-landed-97c4a4c1.md`.

## Round 3's candidate

`claude/KRN-landing` e4fd3b3768fb, on `main` 23ff2b6, merges two requests:
1. **EFF**, `claude/EFF-effector-agent` 732c264 (1507Z).
   - Its merge conflicted with `main` in `issues/README.md`'s finished table. I kept both sides' rows; nothing to do.
2. **KRN**, `claude/youthful-mendel-mf1soy` 3d329d4 (1606Z): the `linkupdate` suite, an update put through the link (351-KRN-0141).
   - It joins "x86: files and block writes".
   - My first request also changed the aarch64 line that EFF changes, and that conflict in code would have returned EFF's request. I took that change out. It comes in a later round, on top of EFF's line.

## The machines (preset `full`)

i9, un-rizen (lanes `KRN` and `KRN-2`) and PCU, as in round 2. Please run nothing heavy of your own there until the answer is in `LANDING/`. It takes about 1.5 hours.
