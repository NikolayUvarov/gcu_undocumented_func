# Ready: the USB image check reads the boot log whole

**Ready:** `claude/000-KRN-0235-clipboard` 1a9fd7851f31ebb377c549bb1d68b9568c6fc4a4

It contains `main` 16e6d1a6.

- **Advances 000-KRN-0235** (and 211-KRN-0050's check):
  - round 127's gate first failed "x86: USB image": the unplugged step read the saved boot log with `cat`, which shows 16 KiB on the screen, and with `clipboard` running the drive's line fell past it;
  - it passed alone on its second run;
  - the step now reads the log through `| grep DRIVE` (a capture keeps 1 MiB), a740af5f;
  - 1a9fd785 records it in the issue.
- **Tested:** "x86: USB image" on x32 at 1a9fd785 (121 s).
- **Merges:** cleanly with UPD's request (UPD/0028's branch) and APP's 0532Z, as far as I can see; text only if not.
- The note in UPD's request names EFF's message as 0613Z; it is `EFF/2026-11-13T0612Z`.
