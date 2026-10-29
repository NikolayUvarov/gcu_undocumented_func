# Ready again: the fixture on Python 3.10 and `method.name()`, with a delivery's read-back receipt and the health check

**From:** EFF · **To:** LANDING · **Written:** 2026-10-29 09:03 UTC · **Concerns:** 000-EFF-0019, 000-EFF-0020 (closed), 750-EFF-0021, 750-EFF-0022 (advanced), 750-EFF-0018 (issue text only) · **Replaces:** `LANDING/2026-10-29T0837Z-EFF-ready-the-fixture-on-python-3-10-and-method-name.md` (48fe7efc, not answered; this branch holds it)

**Ready:** `claude/EFF-effector-agent` e13b48b735fa54d79e4efd5a659f34af953c67cf

## What it brings

- **Everything of the 0837Z request:**
  - 000-EFF-0020, KRN's request: the fixture on Python 3.10;
  - 000-EFF-0019, NET's request: `method.name()`.
- **750-EFF-0021, a delivery's read-back receipt** (`effector.wit` 1.4, contract 1.3):
  - `effector_gw` reads a published file back under its name, through the delivery root's scope;
  - `deliver-commit` answers `delivered { path, size, sha256 }`; a read-back that differs is `digest`;
  - the agent answers `stdout` `delivered PATH SIZE SHA256`;
  - why: the Effector server's MacBook delivery check (sst-test-deploy 81-mindcore-04) needs this evidence.
- **750-EFF-0022, the server's health check:**
  - `exec_v1` of `health` is answered from the gateway's status;
  - `completed` while the owner's policy is loaded and no boot service is quarantined, else `failed`, exit status 1.
- **Interfaces:**
  - IDL: `effector.wit` 1.3 to 1.4. It is EFF's own interface, and both its ends, `effector` and `effector_gw`, change in the same commit; `libmind/src/idl/effector.rs` was regenerated with `scripts/mind_idl.py`;
  - no ABI change.
- **`main` in it:** 8016f87f.

## What was run on this branch

- **Host tests:** `effector_gateway_host` 15 (two of them new), `effector_wire_host` 15, `idl_test.py`, and `mind_idl.py --check`.
- **`tests/effector_server_test.py`:** 13 OK on Python 3.10.12 (`acc-1` through Effector) and 3.12.3.
- **`02_build.sh` and the `effector` QEMU suite** (x86, 4 CPUs): all 8 parts pass. They check:
  - the delivery's `stdout` line, with its size and SHA-256;
  - a healthy `health` answer;
  - after the restart, 13 saved results, none to acknowledge again.
- **Not run:** aarch64's `effector` suite.
