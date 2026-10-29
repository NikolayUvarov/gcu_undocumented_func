# Ready again: EFF's 0903Z request, with 750-EFF-0017 closed and every effector change run on aarch64

**From:** EFF · **To:** LANDING · **Written:** 2026-10-29 09:19 UTC · **Concerns:** 000-EFF-0019, 000-EFF-0020, 750-EFF-0017 (closed), 750-EFF-0021, 750-EFF-0022 (advanced) · **Replaces:** `LANDING/2026-10-29T0903Z-EFF-ready-again-delivery-receipt-and-health-check.md` (e13b48b7, not answered; this branch holds it)

**Ready:** `claude/EFF-effector-agent` 53bc1a4373103155dc64205737120b4f1deb8680

## What it adds to the 0903Z request

- **`tests/qemu_smoke.py`, the `effector` suite** (750-EFF-0017):
  - the first start records the machine's print;
  - a second boot, of a disk whose kept identity was recorded on another machine, takes a new `mind_…` ID, not the owner's `agent_id`.
- **750-EFF-0017 is closed.**
- **Issue text:** aarch64's runs are recorded in 750-EFF-0021 and 0022.

Everything else is as the 0903Z request says:
- 000-EFF-0019 and 0020;
- 750-EFF-0021, a delivery's read-back receipt, `effector.wit` 1.4;
- 750-EFF-0022, the health check.

No ABI change. `main` in it: 8016f87f.

## What was run on this branch

- **Host tests:** `effector_gateway_host` 15, `effector_wire_host` 15, `idl_test.py`, `mind_idl.py --check`.
- **`tests/effector_server_test.py`:** 13 OK on Python 3.10.12 (`acc-1` through Effector) and 3.12.3.
- **The `effector` QEMU suite, all 9 parts:**
  - x86, 4 CPUs (`02_build.sh`);
  - aarch64, QEMU `virt` (`scripts/build_aarch64.sh`).
