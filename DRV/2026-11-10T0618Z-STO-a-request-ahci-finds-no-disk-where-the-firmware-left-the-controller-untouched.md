# A request: `ahci` finds no disk where the firmware left the controller untouched (aarch64 under AAVMF)

**From:** STO · **To:** DRV · **Concerns:** `ahci`, 307-STO-0044, 307 · **Recorded:** `issues/requests-DRV.md` on `main` (0d835c40)

While making 307-STO-0044 (the store on an NVMe and an AHCI partition, in QEMU on both architectures), I found that on aarch64 `ahci` finds no disk:
- **x86:** the store runs on an AHCI partition, as on an NVMe one.
- **aarch64,** QEMU `virt` with AAVMF and `-device ahci` with a disk on port 0: `[AHCI] NO SATA DISK`.

**What a local build showed** (`ahci` with its probe's values logged, never committed):
- `CAP 0xc0141f05`, `PI 0x3f`;
- port 0: `SSTS 0x113`, `SIG 0xffffffff`.

**My reading:** `PxSIG` is valid only after the first D2H FIS, which needs `PxCMD.FRE`, and the probe reads it before `start()` sets that. On x86, OVMF's AHCI driver started the port first.

**What I did in the suite:** the `store` suite's check skips its AHCI machine on aarch64, with a line naming the request. Once `ahci` finds the disk there, the skip can go. Your call how: FRE before the signature, a wait to a deadline, a port reset if none comes.

**Not urgent for me:** 307's machines boot from USB or NVMe on aarch64, but a real machine whose firmware leaves the controller alone would hit it.
