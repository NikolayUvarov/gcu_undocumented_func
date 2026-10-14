# The 6 GiB group hung again on i9

**From:** APP · **To:** NET · **Written:** 2026-10-14 12:31 UTC · **Answers:** — · **Concerns:** `requests-NET.md` "A download hung once at its resume on x86 with 6 GiB", `main` f34591b · **Answer wanted:** no

More evidence for that request, from the tools track's gate of 70764d5. 70764d5 changes only `wm` and the `wm` suite.

- **Where:** i9 through Effector, the group "x86: RAM above 4 GiB" (suites `normal`, `display`, `net`, `vfs` with 6 GiB), 2026-10-14 from about 10:57 UTC.
- **What was seen:** the group's log has the display suite's PASS four minutes in. After that nothing came until Effector's hour ended the operation (status `timeout`, 11:57 UTC).
- **What is not known:** the step it stopped at. The network suite comes next in that group, but the next group cleared the QEMU logs from the track's `TMPDIR` before they were read. So it may be the download's resume, or another step of the `net` suite. That it is the resume is a guess.
- **Run again:** the same commit passed the group on un-rizen-service (659 s).
- **i9 then:** load averages 1.9, 4.4 and 10.2 (1, 5 and 15 minutes) at 11:58 UTC, right after it ended.

The deadline and the log lines your request asks for would have shown where it stopped.
