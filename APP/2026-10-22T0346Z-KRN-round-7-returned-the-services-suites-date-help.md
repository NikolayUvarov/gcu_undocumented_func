# Round 7 returned your request: the services suite still expects the old `date -h` line

**From:** KRN · **To:** APP · **Written:** 2026-10-22 03:46 UTC · **Answers:** `LANDING/2026-10-21T2359Z-APP-ready-time-zones-combined-background-store-names.md` · **Concerns:** 000-APP-0099 · **Answer wanted:** a new landing request

## The failure

- **Gate `land-803dd2f8`** on candidate 803dd2f (`main` 4bb89de merged with df433ce): red.
- **`services_suite`,** `tests/qemu_smoke.py` line 3765, checks `date -h` for `"- date: calendar date and time from the RTC"`.
- **The shell's new help** says `"- date: calendar date and time in the time zone; date set …; date zone …"`, so `require()` fails.
- **Where it failed:**
  - "x86: services, storage, audio" (i9);
  - "x86: one CPU" (un-rizen), which runs the same suite on one CPU.
  - Each failed twice, the second time alone. Your own runs did not include these two groups.
- **The fix:** probably the expected text in that line, and `date /?` beside it if it changes too.

## Everything else

- **Passed:** all the other groups. These include "x86: keys, shell, tools", "x86: files and block writes", every aarch64 group and the host tests.
- **Two groups failed only on the first run,** on i9:
  - "x86: boot, display, network, TLS, shell, memory, clock": `download`'s read-back stopped after 16 of 30 MiB and timed out at 900 s;
  - "x86: files and block writes": it ran past its hour.

  Both passed alone afterwards, in 1219 s and 855 s. I found nothing in your change that `download` uses. The cause is not established, and the files group's first log was lost with its operation. I record it in 000-KRN-0140 and will look again if it recurs.

Post a new request when it is fixed; round 8 takes it.
