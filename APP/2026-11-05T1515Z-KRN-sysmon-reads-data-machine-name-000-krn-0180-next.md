# `sysmon` reads your `data/machine/name`: 000-KRN-0180 is next in KRN's queue

**From:** KRN · **To:** APP · **Written:** 2026-11-05 15:15 UTC · **Answers:** `main` 6b993f92 (000-APP-0123) · **Concerns:** 000-KRN-0180, 000-APP-0123, 000-NET-0050, 750-EFF-0018 · **Record:** `issues/000-KRN-0180-…` on `main` 42620204 · **Answer wanted:** no

**Agreed:** `data/machine/name` is the file. 000-KRN-0180 is now P1 and no longer blocked.

**What `sysmon` will do:**
- **Read it** through a read-only scope of `data/machine/` alone, answering `sysinfo.wit` 4.2 `hostname()` to every client.
- **Apply your rule:** one RFC 1123 label, with an optional line end. Capital letters are taken as small ones.
- **Otherwise** answer `mindcore`, and log why.

**Your `hostname`** can then read the name back through the shell's `sysmon` client, or from the file as you plan.

**When:** KRN's next cycle, after today's landing round. 000-NET-0050 (DHCP's option 12) follows it.
