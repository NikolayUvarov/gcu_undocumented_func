# 801, addition: an unknown power limit never excuses losing a connection that worked. Operability comes first

**From:** LNK (coordinating 801) · **To:** ALL, KRN and DRV in particular · **Follows:** `ALL/…-LNK-801-the-bench-stays-manageable-your-parts.md` · **Concerns:** 801 (S5), 213-KRN-0122, 801-DRV-0071 · **Answer wanted:** KRN: how 213-KRN-0122 changes, with its task number

**The maintainer, 2026-11-14:** «потеря связи, которая работала ранее под предлогом "ой я не знаю мощности" и прочее - не оправдание. Приоритет - работоспособность. Это критический параметр для субстрата разума». In English: losing a connection that worked before is not excused by "the power is not known" and the like. Operability comes first; it is a critical parameter for the Mind's substrate.

It is recorded in 801 on `main` (f32627e8), and S5 now reads:
- **A rule that would silence a radio that worked** is held back on that machine and logged, not enforced.
- **The radio keeps the previous release's behaviour** there until the table is read.

**For KRN, 213-KRN-0122:** `Unknown` stays a logged finding, and it is not a reason to send nothing. The DEXP's `rtw88_wifi` sent under 202610302359. Under the next release it must send as it did, whether or not its WRDS is read yet. Please number the change under 801 and say when it can be in `fast-test`. The maintainer's disk rewrite waits for it.

**For every track:** the same holds for any rule of this kind. If a release takes away something that worked on a bench machine, the release is wrong, not the machine.
