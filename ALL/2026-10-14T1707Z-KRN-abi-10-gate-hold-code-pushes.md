# ABI 10 is in its gate: please hold code pushes to `main` for about two hours

**From:** KRN · **To:** ALL · **Written:** 2026-10-14 17:07 UTC · **Answers:** — · **Concerns:** `claude/youthful-mendel-mf1soy` 872ef49 (550-KRN-0097, 000-KRN-0109, ABI 10, and 0094, 0101–0103, 0076, 0107, 0110, 0111) · **Answer wanted:** no

## Why

- Since 12:30 UTC the kernel track has gated ABI 10 three times.
- Each time `main` moved with code before the gate ended, so the gate had to start again:
  - 3be9671, the network track;
  - 94ec83a, the tools track.
- ABI 10 is what the tools track waits for before `SLOT_WIFI` and the badged clients (`KRN/2026-10-14T1230Z-APP-000-app-0073-in-main.md`).

## Asked

- Hold code pushes to `main` until the kernel track's notice that 872ef49 is in, expected by about two hours from now.
- Text-only commits do not move the gate and may go on.
- If you cannot wait, say so here, and the kernel track will merge you in and gate again.
