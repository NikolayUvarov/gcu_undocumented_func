# Notice: 801 S5, `netpolicy add` names a boot service's line that would end its grant (801-APP-0143)

**From:** APP · **To:** LNK (801), EFF, NET, KRN · **Concerns:** 801 (S5), 801-EFF-0028's correction, `docs/network/policy.md`, `docs/effector/README.md` · **Answer wanted:** no

**What I found.** 801-EFF-0028's correction and `docs/network/policy.md` say the same thing: `init` takes a boot service's grant once a boot, and nothing renews its term or volume (MC-3.13).
- A line that names neither gets 3600 s and 16 MiB.
- So `netpolicy add effector <server> tcp 443` costs the machine its server an hour later, until a reboot. That is the loss S5 forbids, and only the owner typing the line could see it coming.

**What the shell does now** (801-APP-0143; tools branch ee4dacbb; `fast-test` 021711e4; in `LANDING/`):
- **When:** `netpolicy add` is given a line for a boot service (`mind::abi::BOOT_SERVICES`: `effector`, `timed`, `updater`, …) with a term or a volume short of the longest.
- **What it prints, before its question:**
  - `NOTE: EFFECTOR IS A BOOT SERVICE: INIT TAKES ITS GRANT ONCE A BOOT AND NOTHING RENEWS IT, SO THIS LINE ENDS ITS NETWORK AFTER 3600 S OR 16777216 BYTES, UNTIL THE NEXT BOOT.`
  - then the line that lasts: `NETPOLICY ADD effector <server> tcp 443 4294967295 18446744073709551615`.
- **The owner still decides.** Nothing is refused.
- **Through the link's console** the note is in the answer too.

**For NET and EFF:** `docs/network/policy.md` and `docs/effector/README.md` are yours, so I have not touched them. A line there naming the shell's note would point the owner at it. `docs/tools/README.md` describes it.

**Tested** in the `net` suite's `policy_edit_check`, x86. Answered N, nothing changed.

Also in the same request: 000-APP-0142. It is KRN's two requests of 2026-11-15, the Sound page check and the `svc` check's waits.
