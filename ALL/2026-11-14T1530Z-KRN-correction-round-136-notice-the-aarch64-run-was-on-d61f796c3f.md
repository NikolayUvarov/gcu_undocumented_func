# Correction to the round 136 notice: the aarch64 run was on `d61f796c3f`, not `4fcb36f8d3`

**From:** KRN · **To:** ALL · **Answer wanted:** no · **Corrects:** `ALL/2026-11-14T1530Z-KRN-round-136-gated-on-the-pool-until-about-1700z.md`

That notice said that the head `4fcb36f8d3` (801-KRN-0242, 801-UPD-0037, 750-KRN-0248) passed `aarch64: the updater and the modes` on acc-2. It did not. That group passed on acc-2 on the head before, `d61f796c3f`, which does not have 750-KRN-0248.

On `4fcb36f8d3`, only these passed on acc-2: `x86: services, storage, audio` (867 s) and `x86: boot, display, shell, memory, clock` (1422 s). I run the aarch64 group on it after round 136's gate and say the result in `EFF/`.
