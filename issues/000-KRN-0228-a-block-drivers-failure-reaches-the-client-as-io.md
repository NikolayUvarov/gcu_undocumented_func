# 000-KRN-0228 — A block driver's failure reaches the client as `io`, not as `peer`

**Type:** libmind (the block protocol's server side, `libmind/src/block_protocol.rs`, `block.rs`; `idl/block.wit` 2.0) · **Owner:** `KRN` (libmind's core) · **Priority:** P2 · **Status:** in progress (libmind's part made on `claude/000-KRN-0228-block-driver-errors`, 2026-11-10; the drivers move on DRV's branch) · **Blocked by:** — · **Main task:** — · **Roadmap:** track A · **Constitution:** MC-6.4, MC-6.6, MC-12.4

DRV's request of 2026-11-09 (`KRN/2026-11-09T2324Z-DRV-a-request-a-drivers-io-failure-reaches-the-client-as-io-not-peer.md` on `internal-dev-messages`), for its [000-DRV-0064](000-DRV-0064-ata-and-ahci-waits-with-a-deadline.md)'s second criterion.

## Problem

- `block_protocol::Driver::read`, `write` and `flush` return `bool`.
- `serve` answers a `false` as `peer`, as for a restarted driver.
- `block::Device` takes `peer` for a restart: it attaches its buffer again and repeats the request once (MC-6.4, MC-6.6).

So a drive that fails or does not answer looks like a restart:
- the client repeats the request;
- a drive held to a 10 s deadline (`ata` and `ahci`, 000-DRV-0064) costs it about 20 s;
- the failure it then sees is `peer`, not an I/O error.

## Plan

1. **The trait.** `Driver::read`, `write` and `flush` return `Outcome`, `Result<(), DriverError>`. `DriverError` is one of:
   - `Io`: the device failed or did not answer;
   - `Range`: refused before the device, e.g. past its end;
   - `Gone`: no device.
2. **What the client is answered.** `serve` answers them `io` (`ERR_IO`), `invalid` and `not-found`; `peer` is left to the kernel, for a driver that is gone.
3. **The client.** `Device` repeats only after `peer` or `not-found`, as it already does, so `io` reaches the client after one call.
4. **The version.** `idl/block.wit` 2.0 (after STO's 1.2 windows, 307-DRV-0054) names `io` for `read`, `write` and `flush`.
   - A major version: the answers of three existing calls change their meaning, and docs/idl's rule gives that a new major version (MC-12.4).
   - A client and a server of different majors refuse each other (0x81); every client is built from the same tree.
5. **The drivers.** DRV moves the six block drivers (`nvme`, `ahci`, `ata`, `usb_storage`, `virtio_blk`, `ramdisk`) to the trait on its own branch, on top of this commit. The two land in one request, so `main` never has the trait without its drivers.

## Acceptance criteria

1. In `tests/block_host.rs`, for `read`, `write` and `flush`, on the whole device and through a window, a driver's:
   - `Io` reaches the client as `io`;
   - `Range` as `invalid`;
   - `Gone` as `not-found`;
   - nothing as `peer`.
2. `idl/block.wit` 2.0 states it, and the bindings are regenerated.
3. The drivers' change on DRV's branch builds, and the block, disk, store and USB suites pass with it on x86 and aarch64. That is the landing request's evidence.

## Progress (2026-11-10)

- **Made:** libmind's part, at 7de1af60 on `claude/000-KRN-0228-block-driver-errors`, on STO's branch at c3938027.
- **Criteria 1 and 2 are met:**
  - `tests/block_host.rs` passes, 7 of 7;
  - `idl/block.wit` is 2.0, and the bindings are regenerated.
- **The version corrected from 1.3 to 2.0** (a62bf84b's next commit): docs/idl's rule gives a change of an existing call's meaning a new major version, as STO had pointed out.
- **A local check, not evidence for the drivers' change** (MC-12.1). A mechanical port of the six drivers, kept in this session and not pushed, was built in the kernel session's container (QEMU 8.2.2, TCG, 4 CPUs). Three groups passed on it:
  - "x86: services, storage, audio", 738 s;
  - "x86: files and block writes", 1055 s;
  - "x86: NVMe boot disk", 111 s.

  None of them makes a drive fail.
- **Sent to DRV and STO:** `ALL/2026-11-10T0214Z-KRN-the-block-drivers-failure-as-io-is-000-krn-0228-libminds-part-is-ready.md`. The third criterion is DRV's branch's.

## Related

- [000-DRV-0064](000-DRV-0064-ata-and-ahci-waits-with-a-deadline.md): `ata` and `ahci` wait to deadlines; its second criterion needs this.
- [000-DRV-0058](000-DRV-0058-usb-storage-reports-a-device-that-came-back.md): `Driver::returned()`, beside it in `block_protocol`.
- [307-DRV-0054](307-DRV-0054-a-block-client-bounded-to-a-window.md): the windows, which this builds on (block.wit 1.2).
