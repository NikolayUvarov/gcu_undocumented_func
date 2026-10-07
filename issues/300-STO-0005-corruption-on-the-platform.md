# 300-STO-0005 — Corruption on the platform: a damaged block, chunk or name record refused by the running store

**Type:** tests (storage) · **Owner:** `STO` track · **Priority:** P3 · **Status:** open · **Blocked by:** a medium a test can change: a durable block device for the store (`DRV`), or fault injection ([500](500-fuzzing-abi-and-idl.md), `500-ASR-0004`) · **Roadmap:** track B; Assurance · **Constitution:** MC-4.8, MC-12.1

Part of main task [300](300-checksummed-block-store.md); split off from 300-STO-0002, 301 and 302.

## Problem

The host tests show the expected outcome of each kind of damage on a simulated medium:
- a flipped byte in a block is refused at mount and at read;
- a damaged header or a torn write loses only its own record;
- a damaged latest name record is reported, and the version before it is current.

On the platform none of this has run. The store's medium is `ramdisk#1`, and only `blockstore` holds its client (Appendix B.6), so no test program can damage it.

## Plan

- **When the store runs on a durable disk** (a VirtIO or AHCI disk image of its own): the QEMU suite stops the VM, flips bytes in the image on the host, boots again, and checks what `blocks stat`, `get`, `check` and `resolve` answer.
- **Or with the assurance track's fault injection:** a driver that damages what it returns.

## Acceptance criteria

The three kinds of damage above give, on x86 and aarch64, the outcomes the host tests give.

## Related

[300-STO-0003](../issues-done/300-STO-0003-blocks-tool-and-store-suite.done), [docs/storage](../docs/storage/README.md).
