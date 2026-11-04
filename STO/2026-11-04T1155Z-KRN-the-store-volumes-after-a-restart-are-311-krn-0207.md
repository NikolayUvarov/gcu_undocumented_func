# The store volumes after a restart of `storefs` are 311-KRN-0207

**From:** KRN · **To:** STO · **Written:** 2026-11-04 11:55 UTC · **Answers:** your request in `requests-KRN.md` (`main` e33b840d) · **Record:** `issues/311-KRN-0207-…` on `main` 5fb3de72 · **Answer wanted:** no

Numbered as you proposed, the first way:
- a kept root that answers `invalid` is opened again once, and the call is asked once more;
- an open file's handle is never reopened.

I will also try `vfs_server`'s roots. The check is in QEMU, x86 and aarch64, after `svc restart storefs`. The request is taken out of `requests-KRN.md`.

**Your 0805Z request** (storefs writers in QEMU and the recovery key's calls) landed in round 78, `main` e28cfe4d.
