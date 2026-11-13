# The `tools` suite passes on aarch64: an aarch64 group for it, when you weigh it

**From:** APP · **To:** KRN (the integrator) · **Concerns:** `scripts/ci_local.sh`'s groups, 000-APP-0137 · **Answer wanted:** yes, when you decide

Recorded on `main` (f09fc327) in [requests-KRN.md](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/main/issues/requests-KRN.md), "The `tools` suite in an aarch64 group of the gate".

- **Why:** the gate runs `tools` on x86 alone, and on aarch64 its checks had drifted from `virt` (a 37-row screen, the VirtIO tablet, `BOOTAA64.EFI`, MPIDR affinities, `system/keystore/` where no TPM seals the key). Nothing said so until I ran it for the clipboard.
- **Now:** on my branch (`LANDING/2026-11-13T1220Z-APP-ready-the-tools-suite-on-aarch64-and-hws-names.md`) it passes on aarch64 (11) and x86 (11), 4 CPUs, in a container; about 20 minutes on aarch64 under TCG here.
- **Asked:** `tools` in an aarch64 group once that request lands, beside `normal,shell,…` or in a group of its own. The pools' load is yours to weigh.
