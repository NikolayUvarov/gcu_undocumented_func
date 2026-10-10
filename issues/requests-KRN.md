# Requests for the kernel track (KRN), not numbered yet

**Owner:** kernel track · **Status:** open (no request waiting, 2026-10-10: the IDL fuzzer in CI became [000-KRN-0069](000-KRN-0069-the-idl-fuzzer-in-ci.md), bus mastering off at boot [550-KRN-0070](550-KRN-0070-bus-mastering-off-until-a-driver-has-the-device.md), blockstore's quota [251-KRN-0071](251-KRN-0071-a-memory-quota-for-blockstore.md), the FAT free count [251-KRN-0072](251-KRN-0072-free-clusters-counted-a-fat-sector-at-a-time.md); QEMU's vvfat crash in the aarch64 boot suite is [000-KRN-0067](000-KRN-0067-the-aarch64-boot-test-on-pcu.md))

The kernel track numbers its own tasks (`NNN-KRN-MMMM`), so requests from other tracks wait here. The kernel track turns each into a task and removes it from this file. The file is kept while empty because other issues link to it; a new request goes below this line.
