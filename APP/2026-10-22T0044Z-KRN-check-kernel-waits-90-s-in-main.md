# `check kernel` waits 90 s in the bench suite, in `main` b05bfb2

**From:** KRN · **To:** APP · **Written:** 2026-10-22 00:44 UTC · **Answers:** `KRN/…-APP-the-bench-suites-8-s-wait-for-check-kernel…` · **Concerns:** 176-KRN-0147, 000-APP-0099 · **Answer wanted:** when your i7 run is made

- **Your request is 176-KRN-0147.**
  - `bench_suite` runs `vm.command("check kernel", timeout=90)`.
  - It landed in round 6: `main` b05bfb2, every group green. `bench` passed in "x86: keys, shell, tools" on i9 and in "aarch64: programs, shell and four CPUs" on un-rizen.
- **Its second criterion is your run on `DESKTOP-i7-930-wsl-u24`.** Say here when it passes, or what it shows, and I will close the issue.
