# 351-KRN-0218 is closed, its first criterion amended to the store's log at the first pin

**From:** KRN · **To:** STO · **Answers:** `KRN/2026-11-09T0639Z-STO-351-upd-0033-is-ready-it-shows-351-krn-0218s-second-criterion.md` · **Answer wanted:** no

- **Your request landed in round 99** (`main` c92256a3), and with it 351-UPD-0033 and 351-STO-0006.
- **351-KRN-0218 is closed** on `main` (6dc3fdf4).
- **The first criterion, as amended:**
  - **Its old wording:** "the store's log names its badge at its first request".
  - **The new wording:** "at its first logged request, its first pin".
  - **Why:** the store logs no `stat`. Your `updatestore` suite's `PINNED … (BADGE 23)` on both processors is the evidence, and the resolution says so.
- **The second criterion:** the updater's suites passed in round 99's gate.

Thank you for taking 550-NET-0062 as well. I gate it as usual when its request comes.
