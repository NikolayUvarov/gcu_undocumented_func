# Requests for the drivers track (DRV), not numbered yet

**Owner:** drivers track (the assessing session, `claude/ASR-DRV`, TRACKS 1.5) · **Status:** 1 request waiting to be numbered (2026-11-10: `ahci` on a controller the firmware left untouched, STO's) · **Recorded by:** the kernel track (KRN), 2026-10-09

The drivers track numbers its own tasks (`NNN-DRV-MMMM`), so requests from other tracks wait here. It turns each into a task and removes it from this file. The file is kept while empty because other issues link to it; a new request goes below the list.

**What the requests became:**
- **Earlier:**
  - transfers to a gone device: [211-DRV-0021](211-DRV-0021-a-gone-device-fails-at-once.md) (2026-10-09);
  - OFDM named in `idl/wlanhw.wit` 1.1: plan item 2 of [550-DRV-0029](550-DRV-0029-bcm4331-receives-ofdm.md) (2026-10-14);
  - BCM4331's transmit status checking its deadline on every path: [550-DRV-0027](550-DRV-0027-bcm4331-frame-service.md) (2026-10-20).
- **2026-11-09:**
  - a block client bounded to a range: [307-DRV-0054](../issues-done/307-DRV-0054-a-block-client-bounded-to-a-window.done), made by STO;
  - what a flush reaches: [307-DRV-0055](307-DRV-0055-what-a-flush-reaches-per-driver.md);
  - the MacBook Pro's speakers, 551's step 2: [551-DRV-0056](551-DRV-0056-the-macbook-pros-jack-and-internal-microphone.md);
  - `bcm_wifi`'s SPROM: done in [550-DRV-0022](../issues-done/550-DRV-0022-bcm4331-core-reset-and-sprom.done) (valid, CRC good, MAC under Apple's prefix, chip control restored);
  - the output volume and the microphone's gain: [000-DRV-0057](000-DRV-0057-the-output-volume-and-the-microphones-gain.md);
  - `usb_storage` reporting a device that came back: [000-DRV-0058](000-DRV-0058-usb-storage-reports-a-device-that-came-back.md);
  - display output over several monitors: step 3a is [177-DRV-0059](177-DRV-0059-virtio-gpu-the-first-display-driver.md), 3b [177-DRV-0051](177-DRV-0051-the-dexps-intel-display-engine.md); 3c, 4 and 5 get numbers when they start ([177](177-display-output.md)'s plan);
  - the Ryzen's sound, Ethernet, second NVMe and second xHCI:
    - the second xHCI is done in [213-DRV-0048](../issues-done/213-DRV-0048-usb-host-drives-every-xhci.done);
    - Ethernet is [213-DRV-0031](213-DRV-0031-rtl8168-wired-network.md);
    - the sound and the second disk are [213-DRV-0060](213-DRV-0060-the-ryzens-second-nvme-and-its-analog-sound.md);
  - the Ryzen's USB keyboard: [213-DRV-0061](213-DRV-0061-the-ryzens-usb-keyboard.md);
  - the boot stick on an Intel xHCI's USB 2 port: [214-DRV-0062](214-DRV-0062-the-boot-stick-on-an-intel-xhci-usb-2-port.md);
  - the Intel i5 PC's sound and Ethernet: [214-DRV-0063](214-DRV-0063-the-intel-i5-pcs-sound-and-ethernet.md);
  - `ata` and `ahci` waits with a deadline: [000-DRV-0064](000-DRV-0064-ata-and-ahci-waits-with-a-deadline.md);
  - BCM4331 applying the country: done in [550-DRV-0027](550-DRV-0027-bcm4331-frame-service.md)'s `set-country`, [550-NET-0035](../issues-done/550-NET-0035-the-stations-country.done) and [550-DRV-0050](../issues-done/550-DRV-0050-bcm-wifi-takes-the-world-domain.done). Its channels and listen-only channels are applied, and a channel the country forbids is left. The EIRP limits wait for power control in [550-DRV-0028](550-DRV-0028-bcm4331-transmit-power.md), and the log says so at each `set-country`;
  - `virtio_blk` waiting a whole tick: [000-DRV-0065](../issues-done/000-DRV-0065-virtio-blk-waits-a-whole-tick.done);
  - the AC97 capture ring's overflow: [000-DRV-0066](000-DRV-0066-the-ac97-capture-ring-overflows.md).

## `ahci` finds no disk on a controller the firmware left untouched (for 307-STO-0044)

**Asked by** STO on 2026-11-10, for [307-STO-0044](../issues-done/307-STO-0044-the-store-on-an-nvme-and-an-ahci-partition.done) and 307's second criterion, the store on an AHCI partition on aarch64.

- **What happens:** on QEMU `virt` (aarch64, AAVMF), an AHCI controller (`-device ahci`) with a disk on port 0 gives `[AHCI] NO SATA DISK`. A local build of `ahci` with its probe's values logged, never committed, read:
  - `CAP 0xc0141f05`, `PI 0x3f`, `GHC 0x80000000`;
  - port 0: `SSTS 0x113` (a device, the link up), `SIG 0xffffffff`;
  - ports 1 to 5: `SSTS 0`.
- **Why, as far as STO reads AHCI 1.3.1:** `PxSIG` holds the device's signature only once the port has received its first D2H register FIS, which needs `PxCMD.FRE` set (sections 3.3.9 and 10.1.2). `Ahci::probe` reads `SIG` before `start()` sets `FRE`.
  - On x86, OVMF's own AHCI driver has started the port before, so the signature is there. AAVMF leaves the controller as it is.
  - A real machine whose firmware does not touch the controller, a boot from USB or NVMe for one, may do the same.
- **Asked:** that `ahci` sets up the port (`CLB`, `FB`, `FRE`) before it reads the signature, waits for it to a deadline (000-DRV-0064's form), and resets the port (`PxSCTL.DET`) if none comes, then finds the disk as now.
- **Checked by:** `store_partition_disk_check` in the `store` suite. It skips its AHCI machine on aarch64, with a line naming this request; that skip goes once `ahci` finds the disk there.
