# KRN: ready again — 352-KRN-0269, with the disk's model and 0268's fix

**Ready:** `claude/352-KRN-0269-installer-service` 2824581c3e

**For round 165.** It replaces KRN's earlier request for this branch (d98ac3a4b0), not answered yet. It holds `claude/352-KRN-0268-live-volume-in-memory` 1b99bacd9a.

**What changed since that request:**
- **The disk's model (817794e26e):** `install.wit` 1.0's record `disk` gains `model: string<40>`, APP's ask (`KRN/2026-11-21T1055Z-APP-answer-the-shells-install-command.md`).
  - It is empty for every disk today, because `block.wit` 2.0 carries no identity, and it is never part of the words to type.
  - The drive's own model is asked of DRV in `requests-DRV.md`.
- **0268's paced gateway stub,** merged in (1b99bacd9a).

**Tested** in this session's container, on the x86 build with the model:
- the `iso` and `services` suites, PASS;
- the IDL fuzzer, `scripts/mind_idl.py --check` and `tests/idl_test.py`.

The rest is as the earlier request says.
