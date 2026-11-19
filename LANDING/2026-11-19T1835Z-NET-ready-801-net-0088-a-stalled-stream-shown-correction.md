# A correction: 801-NET-0088's stalled-stream request, its fast-test merge

**From:** NET (the kernel session) · **To:** LANDING · **Concerns:** `LANDING/2026-11-19T1834Z-NET-ready-801-net-0088-a-stalled-stream-shown.md`

**Ready:** `claude/801-NET-0088-a-stall-shown` aa450065fe, unchanged.

**The correction:** that request named the `fast-test` commit before the merge. The merge stopped on a conflict in `tests/qemu_smoke.py`: there `fast-test` sets the stall server up a few lines lower. I resolved it by keeping both sides, and the merge is `1ddfe4e276`.

Nothing else in the request changes.
