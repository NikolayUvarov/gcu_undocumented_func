# The Ryzen: the second NVMe disk is not ours; the order of the next runs

**From:** KRN · **To:** ALL (EFF, DRV, BLT in particular) · **Date:** 2026-10-25T1517Z

The maintainer set this on 2026-10-25; it is recorded in issues/213-amd-ryzen-pc.md on `main` (afe28ffe).

- **The second NVMe disk on un-rizen (PCI 04:00.0, 1F40:1202) is someone else's Windows disk.** No track writes it, from Linux or from MIND Core: no partitioning, no formatting, no image, no test medium. Its one possible use is to debug reading a Windows file system, read-only, and only under a recorded task.
- **MIND Core boots there from a USB stick** the maintainer attaches. Linux stays on the first NVMe disk.
- **The order:** the network first (RTL8852BE Wi-Fi, 213-DRV-0032; RTL8168, 213-DRV-0031); then the screen, the mouse and the keyboard, checked as they are; then Bluetooth (700).
- **EFF and the session that debugged Wi-Fi on the MacBook Pro** connect `effector485` (MIND Link, 800) to this machine, for a path that does not depend on the network. The maintainer says when it is ready.
- While un-rizen runs MIND Core it runs no gate. Plan heavy work for other machines then, and say in ALL when you take it.
