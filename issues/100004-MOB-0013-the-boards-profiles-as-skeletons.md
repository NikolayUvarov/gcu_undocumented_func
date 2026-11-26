# 100004-MOB-0013 — The boards' platform profiles as skeletons, every line "not tested"

**Type:** profile · **Owner:** `MOB` · **Priority:** P2 · **Status:** in progress (2026-11-26: the skeletons written; every line waits for a run on its board) · **Blocked by:** the boards' first boots ([100004-MOB-0002](100004-MOB-0002-the-first-boot-on-the-h5.md), [100004-MOB-0003](100004-MOB-0003-the-first-boot-on-the-h618.md)); the bench's board and its components named ([issues-human, section 14](../issues-human/README.md#14-the-two-allwinner-boards-on-the-bench)) · **Main task:** [100004](100004-two-allwinner-boards-before-the-phone.md) · **Constitution:** MC-1.5, MC-9.1, MC-12.1, MC-12.3

**Asked by:** the kernel session (`requests-MOB.md`, request 2's item 4), turned by the maintainer's word of 2026-11-26 into the bench board's profile: «MOB будет делать работу на стенде - это не телефон, а плата разработки с добавленными компонентами». The PinePhone's profile (`aarch64/PinePhone-0`, [100000](100000-a-phone-the-pinephone-first.md)'s stage 6) waits for the phone.

## Problem

A profile states what MIND Core is, guarantees and does not claim on one platform, with its TCB and evidence (MC-12.1, MC-12.3). The two boards have none. Before a run, the place where a run's results will go is to be ready, and nothing in it may read as a guarantee.

## Plan

1. **`docs/profile/aarch64-orangepi-zero-plus2/` and `docs/profile/aarch64-orangepi-zero3/`,** in the shape of [`aarch64/QEMU-virt-0`](../docs/profile/aarch64/README.md): `README.md`, `tcb.md`, `threat-model.md`, `evidence.md`.
2. **Each line says where it was read** (the catalog, Linux's tree through the host tests, the kernel's code, U-Boot's or TF-A's sources, the maintainer's handoff) and that it is **not tested**.
3. **Where a profile says nothing, it claims nothing:** unlike `aarch64/QEMU-virt-0`, which inherits the x86 profile's statements, these inherit no statement from QEMU (MC-12.1).
4. **A marked place for the bench's components,** to be filled once the maintainer names them, each checked against the board's tree.
5. **`docs/profile/README.md`'s table** names the two skeletons.

## Acceptance criteria

- [ ] Both skeletons, every statement "not tested", "not made" or "host test only", none claimed as a guarantee.
- [ ] The bench's components' place, marked.
- [ ] After each board's first boot: its run in `evidence.md`, and each statement it showed changed from "not tested", with the configuration (MC-12.1). Not done: no board is wired.

## Related

[100004-MOB-0002](100004-MOB-0002-the-first-boot-on-the-h5.md), [100004-MOB-0003](100004-MOB-0003-the-first-boot-on-the-h618.md), [100004-MOB-0012](100004-MOB-0012-the-boards-first-boot-step-by-step-and-its-run-script.md), [docs/profile/aarch64](../docs/profile/aarch64/README.md), [100000](100000-a-phone-the-pinephone-first.md) (stage 6).
