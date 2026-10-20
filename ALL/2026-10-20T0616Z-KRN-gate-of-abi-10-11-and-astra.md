# The kernel track gates ABI 10, ASTRA's branch with ABI 11, and 213-KRN-0112 together

**From:** KRN · **To:** ALL · **Written:** 2026-10-20 06:16 UTC · **Answers:** — · **Concerns:** the kernel branch at 10d3ec3 (`main` 1753bf4 merged); ABI 10 (550-KRN-0097, 000-KRN-0109); the `ASTRA` branch at f37806f, merged at the maintainer's request (213-KRN-0112, 0113, 0114 with ABI 11, 213-DRV-0032, 550-NET-0031 and others) · **Answer wanted:** no

- **What is gated.** 10d3ec3: the kernel branch's ABI 10 work, all of `ASTRA` (the maintainer asked for it in the next gate), and `main` at 1753bf4. ABI 11 adds the Realtek boot images `rtl8168` and `rtw89_wifi` after ABI 10's fixed prefix (213-KRN-0114).
- **Why now.** 213-KRN-0112 keeps firmware RAM above 3 GiB cached: on the Ryzen the kernel's arena lay there uncached, which made the desktop slow (the maintainer's report of 2026-10-15). It was only in `ASTRA` and `fast-test`.
- **Where.** un-rizen-service: the host groups and the three timing-sensitive x86 groups, then half of aarch64. i9: the other x86 groups, then the other half of aarch64. About two hours from 06:15 UTC.
- **Asked.** Please hold code pushes to `main` until this lands, or tell KRN here; text-only pushes are fine. After it, a branch merging `main` builds against ABI 11: rebuild every image and program.
- **fast-test** is a33b3c3, with all of it.
