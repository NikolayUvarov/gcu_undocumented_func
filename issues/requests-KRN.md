# Requests for the kernel track (KRN), not numbered yet

**Owner:** kernel track · **Status:** open (1 request waiting, 2026-10-10; the IDL fuzzer in CI became [000-KRN-0069](000-KRN-0069-the-idl-fuzzer-in-ci.md), bus mastering off at boot [550-KRN-0070](550-KRN-0070-bus-mastering-off-until-a-driver-has-the-device.md), blockstore's quota [251-KRN-0071](251-KRN-0071-a-memory-quota-for-blockstore.md), the FAT free count [251-KRN-0072](251-KRN-0072-free-clusters-counted-a-fat-sector-at-a-time.md); QEMU's vvfat crash in the aarch64 boot suite is [000-KRN-0067](000-KRN-0067-the-aarch64-boot-test-on-pcu.md))

The kernel track numbers its own tasks (`NNN-KRN-MMMM`), so requests from other tracks wait here. The kernel track turns each into a task and removes it from this file. The file is kept while empty because other issues link to it; a new request goes below this line.

## A badge set only by a holder with the right to set it

**Recorded by:** the update track (`UPD`), 2026-10-10, from [351-KRN-0022](../issues-done/351-KRN-0022-updater-grants.done) and [351-UPD-0008](../issues-done/351-UPD-0008-update-zone-in-vfs.done).

### Problem

`CAP_MINT` sets a badge on any unbadged endpoint capability, whoever holds it (`mint` in `kernel/src/scheduler.rs`, MC-3.4). A server that grants authority by badge is therefore only as safe as `init`'s care never to hand out an unbadged client of it: 351-KRN-0022 badged the clients of `init` and `vfs_server` one by one (`BADGE_LIFECYCLE`, `BADGE_READER`) for this reason. Every other badge-checking server depends on the same care, and a new grant can break it silently. Which clients are unbadged today was sent to the maintainer and the kernel session privately, as [SECURITY.md](../SECURITY.md) asks.

### Plan (a proposal; the kernel track decides)

- A right on endpoint capabilities, for example `CAP_BADGE`, that `CAP_MINT` requires to set a badge. A child never carries it unless the mask keeps it, and a badged child never does.
- `init` keeps it on the clients it badges from, and every capability it hands out lacks it.
- A new ABI version and an explicit transition (MC-12.4, 12.7).

### Acceptance criteria

A new `isolation` case: a program holding an unbadged client without the right gets `ERR_INVALID` when it sets a badge, and the suites pass as before.
