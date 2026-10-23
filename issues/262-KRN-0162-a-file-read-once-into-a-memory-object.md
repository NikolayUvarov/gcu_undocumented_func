# 262-KRN-0162 — A file read once into a memory object, mapped read-only by a program and its workers

**Type:** `vfs_server`, IDL (`vfs.wit` 2.4), kernel (the shared-mapping limit) · **Owner:** kernel session · **Priority:** P2 · **Status:** open · **Blocked by:** [262-KRN-0160](262-KRN-0160-large-private-memory-for-a-named-program.md) · **Main task:** [262](262-model-runtime-on-the-machines-full-hardware.md) · **Constitution:** MC-3.13, MC-4.2, MC-11.11, MC-12.4

From the tools track's request of 2026-10-23 in [requests-KRN.md](requests-KRN.md), item 3.

## Problem

- **What a program does today.** It reads a model through `vfs.wit`'s `read`: at most 16 KiB a call, so about 250 000 calls for 4 GB.
- **What it costs.** It needs a heap of the file's size beside the bytes it reads, and each worker of [262-KRN-0163](262-KRN-0163-workers-on-several-cpus.md) would read its own copy.
- **The shared-mapping limit.** A task may map at most 256 MiB of foreign memory (`SHARED_MAX_BYTES`), which a model's weights pass.

## Plan

1. **`vfs.wit` 2.4, a minor version:** `fill: func(file: u32, offset: u64, target: borrow<memory>) -> result<u64, error>` at the end.
   - The program lends a memory object of its own, so the bytes are charged to it (MC-3.13), not to `vfs_server`.
   - `vfs_server` writes the file's bytes from `offset` into the object, as many as fit, and answers how many.
   - No 16 KiB limit: one call for the whole object.
2. **The program then maps the object read-only** and lends it read-only (`CAP_READ`) to its workers. So one copy of the weights is in memory, shared by all.
3. **The program checks the bytes** against the model's hash (262: model files are untrusted input, MC-11.11).
4. **The shared-mapping limit** becomes the larger of 256 MiB and the task's memory quota, so a program and its workers can map the object whole.
5. **Files up to 4 GiB − 1 byte,** FAT32's limit. A larger model is published as several files (GGUF's splits), each read into its own object.
6. **A file shared by several programs** (one copy for all, kept by `vfs_server`) is not part of this task. It waits for a measurement that it is needed.
7. **The store's models** are [251-STO-0015](251-STO-0015-a-model-read-into-memory.md)'s, the storage track's; this task covers files of volumes.

## Acceptance criteria

- [ ] A QEMU suite (x86 and aarch64):
  - a test program fills a 1 GiB object from a file of the model disk in one call;
  - it maps the object read-only, and its bytes match the file's SHA-256;
  - its workers map it read-only, and a write by one of them faults;
  - the time is stated beside that of `read`'s 16 KiB calls.
- [ ] `docs/idl` lists `vfs.wit` 2.4, and `docs/api` the call.

## Related

[262-KRN-0160](262-KRN-0160-large-private-memory-for-a-named-program.md), [262-KRN-0163](262-KRN-0163-workers-on-several-cpus.md), [251-STO-0015](251-STO-0015-a-model-read-into-memory.md), [251](251-model-cache-and-model-disk.md) (the model disk).
