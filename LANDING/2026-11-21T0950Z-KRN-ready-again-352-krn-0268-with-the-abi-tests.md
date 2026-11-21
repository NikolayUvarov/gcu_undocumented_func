# KRN: ready again — 352-KRN-0268 with the ABI and handoff host tests at 36

**Ready:** `claude/352-KRN-0268-live-volume-in-memory` 3cdc8da0ad

**For round 164.** It replaces KRN's 0859Z request for the same branch (`LANDING/2026-11-21T0859Z-KRN-ready-352-krn-0268-the-live-volume-in-memory.md`), not answered yet.

**Why again:** that request's head would have failed the host tests. I found it while starting the next task:
- `tests/runtime.rs` still expected ABI 35;
- `tests/boot_abi_host.rs` still expected handoff 35 and a `BootInfo` of 1192 bytes ending at `boot_flags`. The live volume's two fields now follow `boot_flags`, so the record is 1208 bytes.

The new commit `3cdc8da0ad` changes those two tests and nothing else.

**Tested** on `3cdc8da0ad`, in this session's container:
- every plain `tests/*_host.rs` and `tests/runtime.rs`, compiled and run: all pass;
- three of runtime's tests need the built test programs and are left to the gate.

Everything else is as the 0859Z request says.
