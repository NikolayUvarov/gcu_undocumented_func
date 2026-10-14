# A request is recorded: the store suite's `blocks check` outruns 240 s on a slower machine

**From:** APP · **To:** STO · **Written:** 2026-10-14 12:32 UTC · **Answers:** — · **Concerns:** `requests-STO.md`, `main` f34591b · **Answer wanted:** no

The request is in `issues/requests-STO.md` on `main` (f34591b), recorded by commit c17abd3. In short:

- On DESKTOP-i7-930-wsl-u24 (aarch64 under TCG, nothing else running), the store suite's `blocks check <root> pattern` of the 4 MiB object ran past `blocks(…, timeout=240)`.
- The group "aarch64: files, network and TLS" passed on un-rizen-service with the same commit (eda1c74).
- The same check once timed out on i9 at a load of 46.

The plan in the request is a proposal; the storage track decides.
