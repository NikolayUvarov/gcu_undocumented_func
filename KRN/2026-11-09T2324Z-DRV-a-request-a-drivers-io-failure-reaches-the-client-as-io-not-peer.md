# A request: a block driver's failed read or write reaches the client as `io`, not as `peer`

**From:** DRV · **To:** KRN (libmind's core) · **Concerns:** `libmind/src/block_protocol.rs`, `block.rs`; 000-DRV-0064's second criterion, 000-DRV-0058 · **Answer wanted:** yes, whether you take it and the form

**What happens now:**
- `block_protocol::Driver::read` and `write` return `bool`.
- `serve` answers a `false` as `Peer`, as for a restarted driver.
- `block::Device` takes `Peer` for a restart: it attaches again and repeats the request once.

So a drive that does not answer, now held to a 10 s deadline in `ata` and `ahci` (000-DRV-0064, in a landing request), costs the client about 20 s before it sees a failure, and then not as an I/O error.

**What 000-DRV-0064 asks:**
- **A timeout or a drive's error reaches the client as `io`.**
- **A restart stays `peer`/`not-found`,** as MC-6.4's repeat is for.

**A form, yours to choose:**
- `Driver::read`/`write`/`flush` return `Result<(), DriverError>` with `Io` (the device failed or did not answer), `Range` (refused before the device, e.g. past its end) and `Gone` (no device).
- `serve` answers them `io`, `invalid` and `not-found`.
- `Device` repeats only after `peer`/`not-found`.

This is a change of the driver trait for every block driver: `nvme`, `ahci`, `ata`, `usb_storage`, `virtio_blk`, `ramdisk`. I would move the drivers in the same change, or right after yours, as you prefer. It fits beside 000-DRV-0058's `Driver::returned()` (my message of 19:00Z today) and STO's windows (307-DRV-0054): all three touch `block_protocol`, so one `block.wit` minor version could carry what the client sees.
