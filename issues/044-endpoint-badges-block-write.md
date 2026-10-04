# 044 — Endpoint badges and block write

**Type:** architecture · **Priority:** P0 · **Status:** open · **Blocked by:** — · **Roadmap:** track G (tools plan F8), C8 · **Constitution:** MC-1.1, MC-3.4, Appendix B.6

## Problem

Block drivers serve only reads; writing must be a separate right (B.6). A server cannot tell through which capability a request came, so one endpoint cannot serve read-only and read-write clients differently.

## Plan

- **Badges:** `CAP_MINT` of an endpoint may set a 16-bit badge (`msg[2]`) on a child that has none; `IPC_RECV` reports the badge of the capability the sender used (`msg[1]` bits 16–31). Badges are inherited by copies and cannot be changed. `CAP_INFO` reports the badge.
- **Block protocol:** `BLOCK_WRITE` (sectors from the attached buffer) and `BLOCK_FLUSH`, accepted only from clients whose badge has the write bit; `BLOCK_INFO` reports write protection.
- Drivers: `ata` WRITE SECTORS (0x30) + FLUSH CACHE (0xE7); `ahci` WRITE DMA EXT (0x35) + FLUSH CACHE EXT (0xEA); `usb_storage` SCSI WRITE(10) (0x2A) + SYNCHRONIZE CACHE(10) (0x35), write protect from MODE SENSE(6).
- `init` gives `vfs_server` write-badged clients; nobody else gets them.

## Acceptance criteria

- Isolation fixture: a client without the write badge gets `ERR_RIGHTS` on `BLOCK_WRITE`; badges cannot be changed by minting again.
- QEMU: written sectors read back on ATA, AHCI and USB (raw image disks), host check of the image.

## Related

[docs/tools](../docs/tools/README.md) F8.
