# 213-DRV-0060 — The Ryzen 9 5900HX's second NVMe disk, and its analog sound heard

**Type:** driver · **Owner:** `DRV` · **Priority:** P2 · **Status:** open · **Blocked by:** for the second disk, `init`'s `nvme#1` (KRN: the request of 2026-11-26), then the maintainer's run · **Main task:** [213](213-amd-ryzen-pc.md) · **Constitution:** MC-6.1, MC-12.1

Numbered from the kernel track's request of 2026-10-13 in `requests-DRV.md` ("The Ryzen 9 5900HX machine: sound, Ethernet, a second NVMe disk, a second xHCI"), from the maintainer's first run (`log:hw0003.txt`).

## Problem

What the request named, and where each part is now:
- **The second xHCI** (05:00.4): done in [213-DRV-0048](../issues-done/213-DRV-0048-usb-host-drives-every-xhci.done).
- **Ethernet,** the RTL8168 at 02:00.0: [213-DRV-0031](213-DRV-0031-rtl8168-wired-network.md). It carried the machine's traffic and the Effector agent in November.
- **Sound.** `audio_gw` took the GPU's HDMI audio (05:00.1, 1002:1637), found no output path and gave up.
  - It now tries a second HD Audio function when the first has none: `init`'s part is [213-KRN-0127](213-KRN-0127-audio-gw-gets-a-second-hd-audio-function.md), and `audio_gw`'s `Device::open`.
  - The analog controller (05:00.6, 1022:15E3) has not been heard on this machine yet.
- **The second NVMe disk** (04:00.0, 1F40:1202): `init` lends `nvme` only the first controller (01:00.0), and `nvme` drives one.

## Plan

1. **The sound:** on the Ryzen, with a build of `main`:
   - the log shows `TRYING THE SECOND` and `DRIVING THE SECOND FUNCTION`;
   - `beep` plays through the analog output, to the maintainer's ears.

   If the codec has no path `audio_gw` knows, its dump is read and the path added.
2. **The second disk:**
   - `nvme` takes its controller from `init` by instance, as `virtio_blk#1` does;
   - `init`'s part, `nvme#1` with the second controller's BAR0, is asked of KRN;
   - `vfs_server` and the block store see it as another device.

## Acceptance criteria

On the Ryzen:
- a tone plays through its analog output;
- the second NVMe disk is listed with its capacity;
- its first sectors read the same as Linux reads them.

The machine is the maintainer's: the run needs it booted into MIND Core, and the tone needs ears.

## Progress (2026-11-13): the analog output heard; the second disk still not listed

- **Run:** the Ryzen on release 202611131315 (`main` 20fba2c5), speakers on its analog jack.
- **The output:** `audio_gw` drives HDA's second function: Realtek `10EC:0269`, out pin 0x15 (headphone jack) from DAC 0x2.
  - `beep` (440 Hz and 660 Hz) and `say`, run over the link, were heard by the maintainer at 100 % (0 dB of 88 settings).
  - The first criterion is met.
- **The disks:** only one `nvme` instance runs, `NAMESPACE 1: 250069680 SECTORS`. That is the Patriot disk with Windows' partitions.
  - Linux lists a second one, a Netac 128 GB with Ubuntu, and MIND Core lists nothing of it.
  - The second and third criteria are left.

## Progress (2026-11-26): `nvme` by instance, read-only past the first; init's part asked of KRN

**Built** (`DRV`, this branch):
- **The instance from init's argument** (`nvme/src/guard.rs`): none or `0` is the first, which writes as before. Any other number, or text that names none, is read-only.
- **Read-only at the testing stage.** The second disk holds the Ubuntu install that the Ryzen's other users boot.
  - The instance sends its controller only Identify, Create I/O Completion Queue, Create I/O Submission Queue and Read. Every other opcode of either queue is refused in `submit` before the queue is touched: Write, Write Zeroes, Write Uncorrectable, Dataset Management, Flush, Format NVM, Sanitize, namespace management, firmware, Set Features and the rest.
  - It answers `writable` false, so the serve loop refuses its writes and flushes for every client badge.
  - The drivers' serve loop (`libmind/src/block.rs`) now tells the driver of each such refusal, with the client's badge and PID (`Driver::refused`, a no-op for the other drivers). `nvme` says each one, one line each up to 64, then every 1024th with the count.
  - At start it says that it is read-only, and logs the SHA-256 of its disk's first 2048 sectors (1 MiB) and whether sector 1 is a GPT header.
- **Every instance logs its controller's vendor, model and firmware;** the first instance's other lines are as before.

**Not built here:** `init`'s `nvme#1` (the second controller in PCI order, with the argument `1`) and its place in `STORE_DRIVERS` and in `vfs_server`'s drivers. These are KRN's, and the request was written on 2026-11-26. The DRV owner posts it.
- For a QEMU run before that, the bench branch `claude/DRV-nvme-bench` carries them, in commits titled "Bench only, not for landing". It adds `nvme#1` to a list of init's own, not to `common/abi.rs`.
- It also adds the QEMU check `--suites nvme2` (`second_nvme_check` in `tests/qemu_smoke.py`).

**Tested** (on the maintainer's PC LEG9, WSL2, QEMU; no hardware):
- `tests/nvme_host.rs` (7 tests): the argument's rule, the allow-list against all 256 opcodes of each queue, and the refusals' bound.
- On the bench branch, `--suites nvme2`, x86, 4 CPUs, passed. It runs two machines:
  - **Machine 1:** the system boots from the first NVMe disk. The second disk holds a GPT image of another system.
    - `nvme#1` says it is read-only and lists 131072 sectors.
    - The SHA-256 of its first MiB, read in MIND Core, is the image file's.
    - init lists its 2 partitions and gives `vfs_server` none of the disk.
    - A file written on the first disk is read back.
  - **Machine 2:** the block stand-in for `vfs_server` (`tests/block_app.rs`) writes to every disk it is given.
    - On the first NVMe disk its sectors land.
    - On the second, its 3 writes and its flush are refused, and `nvme#1` says each one with badge `0x1` and the stand-in's PID.
  - After each machine, the second image file's SHA-256 is unchanged.
- **On this branch:**
  - `bash ./02_build.sh` (x86_64) and `scripts/build_aarch64.sh` passed.
  - `scripts/host_tests.sh`, all of it, passed.
  - The x86 groups "NVMe boot disk", "files and block writes" and "services, storage, audio" passed, on 4 CPUs. The last one includes `ahci`.
    - Its first run failed in `audio` on a timing bound, `0.55 < seconds < 0.65` against 0.662. That was while the host tests ran beside it.
    - Rerun alone, the whole group passed.
- These are QEMU runs. They are no evidence for the Ryzen (MC-12.1).

**Not done:**
- The second and third criteria on the Ryzen. They wait for KRN's grant, or the bench image, and the maintainer's run.
- The second disk writable: split to [213-DRV-0105](213-DRV-0105-the-ryzens-second-nvme-disk-writable.md). No instance but the first writes.

## Related

[213](213-amd-ryzen-pc.md), [213-DRV-0105](213-DRV-0105-the-ryzens-second-nvme-disk-writable.md), [213-KRN-0127](213-KRN-0127-audio-gw-gets-a-second-hd-audio-function.md), [213-DRV-0031](213-DRV-0031-rtl8168-wired-network.md), [213-DRV-0048](../issues-done/213-DRV-0048-usb-host-drives-every-xhci.done), [551](551-sound-on-pcs.md).
