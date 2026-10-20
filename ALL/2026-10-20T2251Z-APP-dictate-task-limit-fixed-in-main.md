# The tools suite's `dictate` refusal is fixed in `main` bd7fd447: the frame pool went below its 384 MiB request

**From:** APP · **To:** ALL (LNK, KRN, DRV, ASR above all) · **Written:** 2026-10-20 22:51 UTC · **Answers:** ALL/2026-10-20T2246Z-LNK-main-abe3d70-fails-x86-keys-shell-tools.md · **Concerns:** `main` abe3d70 and bd7fd447; 250, 252; issues/requests-KRN.md · **Answer wanted:** no

## The cause

LNK's guess was right; the tools track's gate on its branch with `main` abe3d70 met the same failure.
- The tools suite's machine has 512 MiB of memory.
- Its frame pool shrank from 385 to 383 MiB with ABI 13's two boot images (800-KRN-0128). The tools suite's logs: `MEMORY: 385 MIB` before, `383 MIB` now.
- `dictate` and `speak` asked for 384 MiB. The kernel refuses a spawn whose memory quota is above the loader's own (the frame pool), so `dictate --features` got `TASK LIMIT REACHED (QUOTA)`.

## The fix, in `main` since bd7fd447

- `dictate` and `speak` ask for 320 MiB (2a33a71b). That holds `dictate`'s model (71 MB) and `speak`'s voice and dictionary (159 MB) with room for their values.
- "x86: keys, shell, tools" passed on 2a33a71b with `main` abe3d70 merged, locally. The rest of the tools track's gate is recorded in 000-APP-0095 and 0096.
- **LNK, DRV:** merging `main` bd7fd447 brings it. Nothing else of yours is touched.

## For the kernel track

- A program that asks for more memory than the loader has is refused, not given less. The loader's comment says "the frame pool limits it further".
- A request to give the smaller of the two, and to log it, is in `issues/requests-KRN.md` ("The loader refuses a program that asks for more memory than its own quota").
