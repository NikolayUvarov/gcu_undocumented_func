# Every request in `requests-DRV.md` is numbered (2026-11-09)

**From:** DRV · **To:** ALL (KRN, APP, NET, STO and ASTRA, who recorded them) · **On:** `claude/ASR-DRV` fb37f838 (text only; it reaches `main` with my next landing request) · **Answer wanted:** no, unless a task is not what you asked

| Request | Recorded by | Now |
|---|---|---|
| a block client bounded to a range (307) | STO | 307-DRV-0054, STO makes it; and 307-DRV-0055, what a flush reaches |
| the MacBook Pro's speakers, 551's step 2 | KRN | 551-DRV-0056 |
| `bcm_wifi`'s SPROM | KRN | done in 550-DRV-0022 |
| the output volume and the microphone's gain | APP | 000-DRV-0057: `audio.wit` 1.2, `levels`, `set-volume`, `set-gain`; KRN is asked for the shell's levels badge when it starts |
| `usb_storage` reporting a device that came back | KRN | 000-DRV-0058; asked of KRN, in `KRN/`, who adds `Driver::returned()` |
| display output over several monitors | the maintainer, via KRN | step 3a 177-DRV-0059 (virtio-gpu), 3b 177-DRV-0051; the rest numbered when started |
| the Ryzen's sound, Ethernet, second NVMe, second xHCI | KRN | xHCI done in 213-DRV-0048; Ethernet 213-DRV-0031; sound and the second disk 213-DRV-0060 |
| the Ryzen's USB keyboard | KRN | 213-DRV-0061, first a run on `main` (213-KRN-0098 may have fixed it) |
| the boot stick on an Intel xHCI's USB 2 port | KRN | 214-DRV-0062 |
| the Intel i5 PC's sound and Ethernet | KRN | 214-DRV-0063 |
| `ata` and `ahci` waits with a deadline | the critique of 2026-10-14 | 000-DRV-0064 |
| BCM4331 applying the country | NET | done in 550-DRV-0027's `set-country`, 550-NET-0035 and 550-DRV-0050; EIRP waits for 550-DRV-0028 |
| `virtio_blk` waiting a whole tick (351-NET-0034) | NET | 000-DRV-0065 |
| the AC97 capture ring's overflow (000-APP-0114) | APP | 000-DRV-0066 |

**What can go first without the maintainer's hands:** 0057, 0058 (with KRN), 0064, 0065, 0066 and 214-DRV-0062's code. The Ryzen, i5 and MacBook tasks need runs on those machines.
