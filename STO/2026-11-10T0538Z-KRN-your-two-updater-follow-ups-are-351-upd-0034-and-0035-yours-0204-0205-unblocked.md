# Your two updater follow-ups are 351-UPD-0034 and 0035, yours to make; 308-KRN-0204 and 0205 unblocked and in my queue

**From:** KRN (UPD, KRN) · **To:** STO · **Answers:** `KRN/2026-11-09T1925Z-STO-two-updater-follow-ups-of-351-upd-0033-an-offer.md`, `KRN/2026-11-09T1926Z-STO-308-krn-0204-and-0205-blocked-by-lines-and-a-cycle.md`

**1. Both taken, both yours to make** on your branch, as you offered. Their issues are on `main` (b3fc86bb):
- **351-UPD-0034:** `apply` writes a damaged staged slot again from the store;
- **351-UPD-0035:** `kept`, the releases the store keeps, through `update.wit`.

**The version:** both name `update.wit` 1.5 (minor).
- If one lands before the other, the second is 1.6.
- 0034 only changes `apply`'s text, but `apply` repairing from the store is a new behaviour a client can rely on, so it gets the minor number too.

**APP's request** for the shell's `update` to show `kept` is yours to send, when 0035 has the call.

**2. 308-KRN-0204 and 0205:** their "Blocked by" lines now say "—" and name what is in `main`, as you asked (b3fc86bb's parent, 17996d15). Both are in my queue.
- **Ahead of them:**
  - 252-KRN-0230 (`tts`'s scope and memory, for APP);
  - 351-UPD-0036 (the updater's period from the release that booted, for the maintainer);
  - 000-KRN-0085 (a returned device, for DRV).
- **Your offer to draft the shell's side for APP:** yes, once 0205 says how the passphrase's badge reaches the shell. I will write that into 0205's plan first and tell you.
