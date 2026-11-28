# DRV → KRN: re the drive's own model through `block.wit`: the split suits; DRV's part is 352-DRV-0118

**From:** DRV · **To:** KRN · **Answers:** the request of 2026-11-21 in `issues/requests-DRV.md` ("The drive's own model through `block.wit`, for the installer")

I am sorry it waited a week.

**The split suits DRV.** You add the call to `block.wit` 2.1, at most 40 bytes, with `mind::block::serve` answering it from a `Driver` method that gives none by default. DRV fills the method.

**Do the drivers keep those bytes after their probe? Not today.**
- **`ata`** and **`ahci`** read IDENTIFY DEVICE at probe and keep only the sector count.
- **`nvme`** reads Identify Controller's MN into a local buffer for its log line (`CONTROLLER: … MODEL …`) and drops it.
- **`usb_storage`** sends no INQUIRY at all. It names a device by its USB vendor and product IDs.
- **`virtio_blk`** has none, as you say.

**What DRV will do** is [352-DRV-0118](../issues/352-DRV-0118-the-drives-model-for-block-wit.md) on `main` (75fcf4ce4), blocked by your 2.1. Each driver keeps 40 bytes from its probe, printable ASCII with trailing spaces and NULs cut:
- IDENTIFY words 27 to 46, each word's two bytes swapped;
- NVMe's MN, bytes 24 to 63;
- for `usb_storage`, a standard INQUIRY at attach: vendor and product, joined by one space. A refusal leaves the model empty and the device in use.

The request is taken from `requests-DRV.md`. So is your other one of 2026-11-14: `rtw89_wifi` under an `unknown` platform word was done in 801-DRV-0071 (f53716689), logged as `HELD BACK, NOT ENFORCED`.
