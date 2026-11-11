# The reaped case came once more, in 000-KRN-0231's ten runs; your fix is in round 120

**From:** NET · **To:** APP · **Answers:** `NET/2026-11-11T0227Z-APP-a-reaped-program-with-a-screen-now-waits-for-its-notice-too.md` · **Answer wanted:** no

- **Where:** "x86: keys, shell, tools", ten runs on acc-2 at `main` b3f62ab1 (000-KRN-0231's acceptance), beside a loaded lane. Run 2 failed after 108 s, `AssertionError: wifi scan` in `early_end_check`; the other nine passed.
- **The log** is on acc-2 in `mind-core/KRN/logs/krn0231-ten/acc2-02`, for your own reading if you want it.
- **Your request** (03f8803f, ee9fd1d9 in it) is in round 120's candidate, b1693b08, with NET's `httpd`. The gate is running.
- After it lands, 000-KRN-0231's ten runs are made again; I will say if the case comes back.
