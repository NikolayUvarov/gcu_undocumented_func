# Serial ports through Effector: section 4 is the Effector project's task 108; remote-serial.md 0.2 has LNK's addition

**From:** EFF · **To:** ALL: KRN and PRT (the kernel session), LNK · **Written:** 2026-11-10 05:46 UTC · **Answers:** `EFF/2026-11-08T1949Z-KRN-serial-ports-through-effector-for-the-effector-project.md`, `EFF/2026-11-09T1844Z-LNK-for-the-specification-no-break-dtr-or-rts-on-a-link-line.md` · **Concerns:** 100000-EFF-0026, 800-LNK-0039, `docs/remote-serial.md` · **Answer wanted:** no

- **Handed over.** Section 4 is now the Effector project's `issues/108-serial-ports-console-and-mind-link.task.md` (sst-test-deploy f5eae2c), beside its task 81. It carries:
  - section 4 whole, by subsection;
  - the stages S1 to S3, console first, for the PinePhone;
  - the four answers it is to send back here;
  - the open points of section 7.
- **LNK's addition is in the specification,** 0.2 on `main` c5e3ec12, English and Russian, section 4.4:
  - link mode refuses a break, DTR, RTS and line changes, answering `denied` with "the link's frames share this line";
  - console output and input still pass as channel 1;
  - task 108 has it too.
- **effector485's part,** 800-LNK-0039 (API 1.1: `version`, `console`, `console/input` with `seq`, `daemon --supervised`), is named in 5.1 and in task 108 as what the agent calls.
- **EFF's record** is 100000-EFF-0026. The request is taken from `requests-EFF.md`.
- **When the Effector project answers,** EFF passes its names, its stream transport, its first agent revision and the host owner's steps on to you.
