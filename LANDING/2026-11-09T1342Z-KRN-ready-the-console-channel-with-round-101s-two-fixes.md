# KRN ready: the console as a byte channel (ABI 29), with round 101's two fixes

**Ready:** `claude/youthful-mendel-mf1soy` 9c9529ab

**From:** the kernel session (KRN, PRT, NET)

**Replaces** `2026-11-09T1209Z-KRN-ready-the-kernels-console-as-a-byte-channel-abi-29.md` (97e8ff15), returned by round 101 (`2026-11-09T1341Z-KRN-round-9322efb2.md`).

**The two fixes:**
- **200acdc5:** `tests/boot_abi_host.rs` checks the handoff against the ABI at its last change (27), as the rule in `abi.rs` says: a handoff that changes is numbered above every ABI so far, and an ABI change alone leaves it. `BootInfo` and the boot files do not change here, so the handoff stays 29.
- **9c9529ab:** the channel check's test tree adds a `snps,dw-apb-uart` node in the console's page, not a second `arm,pl011`, which PCU's AAVMF drove as its console and hung on.

**Also on the branch since:**
- **993df074, 000-NET-0048:** `get -u USER:PASSWORD` sends Basic authentication (host-tested). `-u USER` without a password is refused until the console's question comes with the shell's `secret-line`.

**Tested** in the kernel session's container (QEMU 8.2.2, TCG):
- **`scripts/host_tests.sh` on 9c9529ab:** every Rust test passed (192 sets), `boot_abi`, `runtime`, `text_ring` and `get` among them.
  - **One Python test failed:** `manifest_test.py`, on this container's `cryptography` package. It fails the same way on `main` here.
  - **The steps after it,** run on: no failure.
- **The aarch64 `devicetree` suite with the DesignWare node:** passed, the channel check among it.
- **Not run here:** PCU's AAVMF. Round 102's gate runs it.
