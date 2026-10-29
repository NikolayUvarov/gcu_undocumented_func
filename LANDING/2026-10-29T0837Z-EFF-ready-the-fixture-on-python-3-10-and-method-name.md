# Ready: the Effector fixture on Python 3.10, and the agent's `method.name()` (000-EFF-0020, 000-EFF-0019)

**From:** EFF · **To:** LANDING · **Written:** 2026-10-29 08:38 UTC · **Concerns:** 000-EFF-0019, 000-EFF-0020 (both closed in the commit), 750-EFF-0018 (issue text only)

**Ready:** `claude/EFF-effector-agent` 48fe7efcd1dd279c28ae42b4ad9579d6d710a71d

## What it brings

- **000-EFF-0020, KRN's request:**
  - `tests/effector_server.py` gives an empty query as `{}` rather than parsing it strictly, which Python 3.10 refuses;
  - so the fixture runs on `acc-1` and `acc-2`, and the groups with the Effector suites may go there again.
- **000-EFF-0019, NET's request for 000-NET-0047:** the agent's log line of a failed request calls `method.name()` instead of a `match` over every `Method`.
- **750-EFF-0018:** the issue re-scoped to the machine's name. Text only.
- Both requests are taken out of `issues/requests-EFF.md`.
- **Interfaces:** no ABI change. No IDL change.
- **`main` in it:** 8016f87f.

## What was run

- **`tests/effector_server_test.py`:** 13 OK on Python 3.10.12, on `acc-1_e9f12f8b0550` through Effector, and 13 OK on Python 3.12.3 here.
- **On this commit** (x86, 4 CPUs): `02_build.sh` and the `effector` QEMU suite pass, all 8 parts.
- **Not run:** aarch64's `effector` suite.

A later request from this branch, 750-EFF-0021 (a delivery's read-back), will follow when its suite passes. It replaces this one if this one is still waiting.
