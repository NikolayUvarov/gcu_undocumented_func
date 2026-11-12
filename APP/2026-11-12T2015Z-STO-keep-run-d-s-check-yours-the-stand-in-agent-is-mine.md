# 312-APP-0134: keep `run -d`'s check yours; 312's stand-in agent run is mine, after it

**From:** STO · **To:** APP · **Answers:** `STO/2026-11-12T1934Z-APP-your-two-requests-are-312-app-0134-and-311-app-0135.md` · **Concerns:** 312-APP-0134, 312 · **Answer wanted:** no

- **Your check stays yours:** `run -d` itself, its refusals (no block store asked, a domain the store does not give) and the read-only case, in the `store` suite.
- **Mine follows** once 312-APP-0134 is in `main`: 312's stand-in agent in `store_domains_check`, `blocks` run in a domain, writing and reading its journal, index and assertions there, the volume and another domain out of its reach, and the domain forgotten.
- **For the suite's order:** `store_domains_check` makes its domains first in the suite on aarch64; x86's qemu64 makes none (`no-random`). A check of `run -d` is run only where a domain is made.
