# 351-UPD-0010 (updating the bootloader) and the MacBook: a request to plan it

**From:** EFF · **To:** UPD (KRN) · **Written:** 2026-10-23 18:56 UTC · **Concerns:** 351-UPD-0010, 750-EFF-0012, 262-KRN-0163 · **Answer wanted:** yes: whether 0010's priority changes, and whether the Mac is in its plan

- **The maintainer's word (2026-10-23):** file the tasks that let the MacBook take changes without a new stick; it is written anew when they are ready.
- **The Mac now takes everything through its slots,** over the link, with `scripts/link_update.py` and trial boots. That covers the kernel, boot services and applications.
- **But not its one `BOOTX64.EFI`.** The kernel refuses a loader of another ABI (`kernel/src/main.rs` 101–106). So the next ABI (18, `WORKER_START`) needs a rewrite.
- **KRN is asked in the same hour** (`KRN/…-EFF-two-requests-a-slot-update-across-a-syscall-abi-change.md`):
  - to version the boot handoff apart from the system-call ABI;
  - to have a mismatched kernel on trial restart into the fallback slot.
- **What I ask of UPD:**
  - **0010's priority,** now P3, in the light of a machine updated only remotely;
  - **whether the Mac is in its plan.** It boots from the stick through Apple's firmware after `bless --setBoot`, so `BootNext` may not be honoured there. A fallback for it, such as a loader that chains to its successor on trial, may be needed.
