# 06 — Apple's platform security: what MIND Core can use on Intel, AMD and ARM

**Revision:** 2026-10-09, commit `5d036c2`. **Why:** the Apple Silicon track (`APL`) is parked, but Apple's security architecture is the most complete public reference for a machine whose software trusts nothing it has not checked. This file records each of its mechanisms, the closest equivalent on the platforms MIND Core runs on (x86-64 with UEFI, aarch64 with UEFI), what MIND Core has today, and what it could take.

**How to read it.** The "MIND Core today" column says only what the code does, with a file or task to check it. Everything under "Could take" is a **proposal**, not a guarantee (MC-12.3). It becomes work only as an issue, and a guarantee only once tested on a stated configuration (MC-12.1, 12.2). Apple's own facts come from the sources at the end. Apple does not publish every detail, so where a description comes from outside research, it is marked as such.

## 1. Boot

| Apple | What it does | Equivalent here | MIND Core today | Could take |
|---|---|---|---|---|
| **Chain of trust**: Boot ROM → iBoot → kernel | Each stage verifies the signature of the next before running it | UEFI Secure Boot (`db`, `dbx`, `KEK`); below it the OEM-fused Intel Boot Guard or AMD Platform Secure Boot, which are not ours to set | The bootloader checks `MANIFEST.SIG` (Ed25519) and every boot image against the manifest before loading it (350-UPD-0003, `bootloader/src/verify.rs`). Secure Boot with our own keys exists (351-UPD-0012). | — |
| **Every executable signed** (code signing; the Signed System Volume seals all system files) | Nothing runs, and no system file is read, unless it matches a signed hash | The same manifest, used past the bootloader | The manifest lists every shipped file with its SHA-256, programs and models included. Only the bootloader checks it, and only for the kernel and boot services. **`loader` starts programs without checking them** (`loader/src/main.rs` has no manifest check), so a program changed on the disk runs. | **P1:** `loader` checks each program's hash against the manifest the bootloader verified (its digest is in `BootInfo.boot_slot.manifest`; `vfs_server` already finds that manifest by it, `vfs_server/src/main.rs:414–465`). Models and other system files are checked when read, by CID (the block store is content-addressed). |
| **Signed System Volume (SSV)**: a Merkle tree over the system volume, root hash in the signed manifest, mounted as a read-only snapshot | The system cannot be changed in place, even by root | A read-only system zone plus content addressing | The boot root zone is read-only except `data/` (`vfs_server`, Zone::BootRoot). The block store addresses blocks by hash (300). | The release's file tree as a Merkle tree in the block store, its root in the manifest, so any file can be verified alone. |
| **Personalised, non-replayable manifests** (Image4 bound to the device ID and a boot nonce) | An old, vulnerable but validly signed system cannot be put back | A version floor in the TPM's NV memory (a monotonic counter) | A/B slots with a trial boot (351-KRN-0014). The version floor is planned (351-UPD-0011, open). | Finish 351-UPD-0011, so the bootloader refuses a manifest below the floor. |
| **LocalPolicy** per installed OS: which keys and kernels are accepted, signed by the Secure Enclave, changed only in recoveryOS after the user proves presence | Lowering security is a deliberate, local, logged act | A boot policy sealed to the TPM; physical presence = a key held at power-on plus a confirmation on the screen | Whether the test key is accepted is fixed in the build. The launch record says "THE TEST KEY" when it was used (350-UPD-0004). | A boot policy file (keys accepted, test kernels, debug) that the bootloader reads, sealed to the TPM. Changing it needs physical presence. The launch record names it. |
| **Boot Progress Registers**: one-way registers record the boot mode (normal, recovery, DFU); the Secure Enclave gives out data keys only in the expected mode | A recovery or debug boot cannot reach user data | TPM PCRs: the bootloader extends a PCR with the mode; keys are sealed to normal-mode PCR values | No TPM use. | Part of measured boot (section 4). |
| **Paired and fallback recoveryOS** | The machine can always be repaired, even when the main system is broken | A rescue slot | Slots A and B. A trial boot that is not confirmed falls back (351-KRN-0014). | A minimal read-only rescue slot that always boots and can fetch or restore a release. |

## 2. The kernel while it runs

| Apple | What it does | Equivalent here | MIND Core today | Could take |
|---|---|---|---|---|
| **Kernel Integrity Protection** (KIP; KTRR in outside research): after boot the memory controller refuses writes to the kernel's code region | Even a kernel bug cannot rewrite kernel code | No memory-controller lock on PCs. The closest are W^X page tables plus CR0.WP, and keeping CR4 and CR0 pinned. Stronger (HVCI-like) needs a monitor below the kernel using EPT or stage 2. | NX is required and set on non-code pages; CR0.WP is set (`kernel/src/arch/x86_64/mmu.rs:98–122`). On aarch64 non-code pages are UXN/PXN (`kernel/src/arch/aarch64/mmu.rs:23–39`). | Check that kernel text is read-only and kernel data no-execute on both architectures, and pin CR0/CR4. A thin monitor (VT-x EPT or EL2 stage 2) that keeps kernel text read-only is long-term. It costs a little on every page-table change, not a virtual machine for programs. |
| **SMEP/SMAP** equivalents (Apple: PAN and PXN in the kernel) | The kernel never executes, nor by mistake reads, a program's memory | x86: SMEP (CR4.20), SMAP (CR4.21), UMIP (CR4.11); ARM: PXN on user pages, PAN | **Done (000-KRN-0039):** x86 sets SMEP, SMAP and UMIP where CPUID offers them; aarch64 sets PAN, user pages being PXN already. The kernel reaches program memory through physical frames (`paging.rs` `readable`/`writable`), so nothing else changed. The boot line `PROTECTION:` and the hardware report say what was set. | Checked in QEMU: the `isolation` suite's `sgdt` case, and a test kernel whose read of and jump into a program's page both fault. |
| **Page Protection Layer / Secure Page Table Monitor** (SPTM, A15/M2 and later): page tables writable only by a small monitor, the single authority over what each physical frame is | A kernel bug cannot map memory it should not | One module that owns frame types (program, page table, kernel, DMA) and checks every mapping; page-table frames read-only outside it | `frames.rs` records which frames are the allocator's (`owns`). Page tables come from that allocator (`paging.rs`), but any kernel code could write them. | Frame types with one authority in the kernel, and page-table frames mapped read-only except inside the paging module. |
| **Pointer Authentication** (PAC, ARMv8.3) and **branch target identification** | Return addresses and function pointers cannot be forged | ARM: FEAT_PAuth, FEAT_BTI (where the core has them). x86: CET shadow stacks (Intel 11th generation, AMD Zen 3 and later) and IBT (Intel 12th generation and later) | None. | **P2:** build with `-Z branch-protection=pac-ret,bti` (aarch64) and CET (`-Z cf-protection`, x86). Enable them in the kernel and for programs when the CPU reports them, so programs on older CPUs run unchanged. |

## 3. Memory safety

| Apple | What it does | Equivalent here | MIND Core today | Could take |
|---|---|---|---|---|
| **Typed allocators** (`kalloc_type`, `xzone malloc`): objects of different types never share memory | A use-after-free cannot make one type's memory into another's | Per-type slabs in the kernel and in `libmind`'s heap | The kernel is Rust. Kernel objects (tasks, capability spaces, page tables) come from the frame allocator (171-KRN-0032); the kernel heap is one shared arena. | Separate pools for the kernel's object types, with guard pages between them. |
| **Memory Integrity Enforcement** (MIE, A19/M5): Enhanced Memory Tagging (EMTE) checked synchronously, typed allocators, and tag confidentiality (protection against speculative leaks) | Overflows and use-after-free trap at once, in hardware | ARM MTE (Armv8.5, some recent cores); nothing equivalent on x86 (LAM only frees pointer bits) | None. | On aarch64 boards with MTE, tagged program heaps in `libmind`. Later. |
| **Zero on free** | Nothing of a previous owner stays readable | — | Frames are zeroed when allocated (`kernel/src/frames.rs:55–58`). A driver's DMA region is cleared before it restarts (MC-6.3). | — |
| **ASLR / KASLR** | Addresses are not known in advance | Randomised placement from RDRAND or RNDR | None: programs and the kernel load at fixed addresses. Programs are already position-independent (static PIE). | **P2:** random bases for program images, stacks and heaps (the loader); the kernel's base later. |

## 4. Keys and data

| Apple | What it does | Equivalent here | MIND Core today | Could take |
|---|---|---|---|---|
| **Secure Enclave**: keys live in a separate processor and never leave it | A compromised system cannot copy keys | TPM 2.0: discrete, or firmware (Intel PTT, AMD fTPM) | `keystore` makes the device key from RDRAND at boot and keeps it only in its memory, so it is new every boot. A persistent device key is planned (351-NET-0005). | `keystore` seals the device key with the TPM to the boot measurements (PCRs). The identity then persists, and is unusable after the system is tampered with. |
| **Sealed Key Protection**: the key-encryption key is derived from the boot measurements and the boot policy (M1–M4) | Changing a measured component or lowering security makes the data unreadable | **Measured boot**: the bootloader extends PCRs (EFI_TCG2_PROTOCOL) with the manifest's digest and the launch record before exiting boot services | The launch record exists (manifest digest, key, images checked, slot) but nothing measures it. | **P1, small:** the bootloader extends a PCR with the manifest digest and the boot mode. That gives later work (sealing, attestation, the version floor) something to rest on. |
| **Data Protection classes**: per-file keys wrapped by class keys, some available only while unlocked; **effaceable storage** erases by destroying keys | Fine-grained encryption; instant wipe | Per-object keys in the block store and `data/`, wrapped by a TPM-sealed class key | No encryption at rest. | With the storage track (`STO`): per-object keys; erase = forget the key. |
| **Attempt limits** in the Secure Enclave | Guessing a passcode is slow and capped | TPM dictionary-attack lockout | — | Use it for any passphrase that unlocks keys. |
| **Attestation** (App Attest, DeviceCheck) | A server can check what booted | TPM quote over the measured PCRs | — | With measured boot: `keystore` can answer with a TPM quote of the launch record. |

## 5. Drivers and DMA

| Apple | What it does | Equivalent here | MIND Core today | Could take |
|---|---|---|---|---|
| **DART**: every DMA-capable device is behind its own IOMMU, default deny; DMA protection for Thunderbolt and PCIe | A device, or its driver, reaches only the memory it was given | Intel VT-d and AMD-Vi (ACPI DMAR and IVRS tables), with interrupt remapping | None. The profile says DMA drivers are in the TCB, and isolation from them is not claimed. VT-d is roadmap item III-4 (ROADMAP K2). | **P1:** IOMMU domains per driver, default deny, the DMA region a driver is granted mapped in its domain only. This shrinks the TCB more than any other item here. It is also needed before GPU drivers (large DMA) are trusted with anything. |
| **DriverKit**: drivers in user space | A driver crash or bug is not a kernel bug | — | Every driver runs in ring 3 / EL0 with only the BARs, IRQs and DMA regions granted, and restarts after its device is quiesced (MC-6.3). | — |
| **Camera and microphone indicator**, kept outside the kernel (Exclaves) | An app cannot hide that it records | An indicator drawn by the compositor, which programs cannot draw over | A capture dot shows while the screen is recorded (issue 165). | The same dot whenever the camera (`video_gw`) or audio capture is open, drawn by the compositor. |

## 6. Isolation and attack surface

| Apple | What it does | MIND Core today | Could take |
|---|---|---|---|
| **Exclaves**: sensitive functions moved out of XNU into a separate world run by a small L4-family microkernel (outside research: the "CL4" Secure Kernel), reached only through defined interfaces | Even a kernel compromise does not reach them | MIND Core is itself a capability microkernel. Services talk over typed interfaces (`idl/*.wit`), and capabilities are delegated, narrowed and revoked. Apple's direction confirms the design. | Keep services small. Put the most sensitive ones (`keystore`) on the fewest dependencies and the shortest interfaces. |
| **Entitlements in the code signature** | What a program may ask for is part of what is signed | A program's `.mind_request` section is listed in the signed manifest and grants nothing by itself (MC-3.11) | — |
| **Lockdown Mode**: an opt-in mode with less attack surface | Fewer features, fewer ways in | — | A strict boot profile: no legacy drivers, no test key, a fixed program list, only the network the policy names. Selected by the boot policy (section 1) and the service configuration (173). |
| **Rapid Security Responses, cryptexes**: small, separately sealed updates | A fix ships without a full release | A/B slots, a signed manifest, the block store (351, 300) | Components (models, programs) with their own signed manifests, updated without a full slot. |

## 7. Suggested order (proposals)

**P1 — cheap and large:**
1. ~~SMEP and UMIP now; SMAP and PAN after the audit (kernel track).~~ Done: 000-KRN-0039.
2. `loader` checks every program against the signed manifest (kernel track).
3. Measured boot: the bootloader extends a PCR with the manifest digest and the boot mode (porting or update track).
4. IOMMU domains per driver, VT-d and AMD-Vi (kernel and drivers tracks; roadmap III-4).

**P2:**
- CET, PAC and BTI where the CPU has them;
- ASLR for programs;
- `keystore` sealed to the TPM (with 351-NET-0005);
- the version floor (351-UPD-0011);
- a boot policy that needs physical presence to change.

**P3:**
- typed kernel pools;
- a page-table authority;
- MTE on ARM boards;
- encryption at rest with per-object keys (`STO`);
- a strict boot profile;
- a rescue slot.

## Sources

- Apple Platform Security guide: [Operating system integrity](https://support.apple.com/guide/security/sec8b776536b) (KIP, SPTM, MIE), [Sealed Key Protection](https://support.apple.com/guide/security/secdc7c6c88e/web), [the guide itself](https://support.apple.com/guide/security/secf5549a4f5/web).
- Apple Security Research: [Memory Integrity Enforcement](https://security.apple.com/blog/memory-integrity-enforcement).
- Steffin and Classen, [Modern iOS Security Features — SPTM, TXM and Exclaves](https://arxiv.org/pdf/2510.09272) (arXiv, 2025): outside research.
- Brandon Azad, Project Zero, [KTRW: the journey to build a debuggable iPhone](https://projectzero.google/2019/10/ktrw-journey-to-build-debuggable-iphone.html) (KTRR as described from outside).
- Asahi Linux: [boot process guide](https://asahilinux.org/docs/alt/boot-process-guide/) (why Apple Silicon is parked here).
