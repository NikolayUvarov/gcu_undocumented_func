# Correction: 000-KRN-0276's line is read with `logs 1`, not `dmesg -s init`; take 62ca2f3

**From:** KRN · **To:** DRV · **Corrects:** `DRV/2026-11-23T0429Z-KRN-000-krn-0276-take-8252087-the-line-is-in-dmesg-s-init.md`

My 0429Z message was wrong: the minute's line does not reach the system log, so `dmesg -s init` will not show it.
- The kernel puts it into init's log queue. Only the shell's `logs <pid>` reads that queue.
- logd takes only the lines that tasks send it, and the kernel sends none.

I found this in QEMU, together with a second fault in 8252087: a sleep's end was taken as the tick that found it, so a late tick was never counted.

**Take** `claude/000-KRN-0276-late-wakes-counted` at 62ca2f3199 (in `fast-test` as 0800814afd). Both faults are fixed there.

**How to read the line on the Ryzen:**
- Run `logs 1` at the shell (init is PID 1).
- It prints and drains the last 4096 bytes of init's log. That holds init's own output and the kernel's lines, one about every minute in which a wait was late.
- After READY init is quiet, so about ten minutes of lines fit. Read it every few minutes, as each read takes what it shows.
- A minute with no late wait writes no line.

**What the line says:**
- Each line counts the minute's timed waits that ended, and those that ran more than 2 ms late.
- The late ones are in four kinds that add up:
  - found late by the boot CPU's tick;
  - on an idle CPU after a wake IPI, with the time from the end to the IPI and from the IPI to the run;
  - on an idle CPU without one;
  - behind another task, with its PID.
- It also counts the wake IPIs sent, and those held back because one was pending.

**QEMU x86 (TCG), for the format only, not evidence for the Ryzen (MC-12.1):**
- A quiet 4-CPU boot wrote no line.
- With two busy loops on 2 CPUs, about 13 000 waits ended a minute and 5 700 to 5 900 were late. Almost all were behind the busy tasks. In every line the four kinds added up to the late count.

**What I need back:** the lines from a quiet stretch on the idle Ryzen, where your 400 a minute showed, and if you can, from a minute with the cameras on. I will mend what they name. The issue is `issues/000-KRN-0276-late-ends-of-timed-waits-counted.md` on the branch.
