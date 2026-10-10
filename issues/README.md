# issues/ — working tasks

## Rules

1. **`issues/` holds only the tasks that are being worked on or scheduled for the near term.** Long-term direction lives in [ROADMAP.md](../ROADMAP.md); a roadmap item becomes an issue here when work on it is about to start.
2. **A finished task leaves this directory.** When its acceptance criteria are met, move it with `git mv` to [`issues-done/`](../issues-done/) and change the extension to `.done`, in the same commit as the work (or right after it). In the moved file:
   - append ` — done` to the title and set `Status: done (YYYY-MM-DD)`;
   - add a `## Resolution` (or `## Done`) section saying what was done and where (files, tests);
   - fix relative links (`../issues/…md` for open tasks, `….done` for finished ones).
3. **A task that became irrelevant** (replaced by another design or by a different task) is moved the same way with status `superseded` and a resolution naming what replaced it. Unfinished remainders are split into a new issue rather than keeping the old one open.
4. **One file per task**: `NNN-TRK-MMMM-short-name.md` — `NNN` the main task it belongs to (`000`: none), `TRK` the track's code, `MMMM` the track's own counter — or `NNN-short-name.md` for a main task (numbered from the track's range). Numbers are never reused; numbers given before this scheme (`158`, `u015`, …) stay. Format: title, metadata line (Type · Priority · Status · Blocked by), Problem, Plan, Acceptance criteria, Related. Every task names the Constitution articles or roadmap item it serves.
5. Update the tables below in the same commit.

Tasks that need a person (repository settings, legal decisions, coordination of agent sessions) are in [issues-human/](../issues-human/README.md).

## Open tasks

Tracks work in parallel; open tracks can be taken now. Their codes, ranges, owners, branches and starting tasks are in the registry [TRACKS.md](../TRACKS.md). A task is numbered `NNN-TRK-MMMM` (main task, track code, the track's own counter); a request to a track with an owner goes to `requests-<TRK>.md`, and a change one's own task needs in an open track is made as that track's task ([AGENTS.md](../AGENTS.md), section 5); ABI changes are made only in `KRN` tasks.

| № | Task | Type / owner | Priority | Blocked by | Roadmap |
|---|---|---|---|---|---|
| [175](175-audit-2026-10-09.md) | The 2026-10-09 audit's eight findings, fixed in their tracks (main task; routing: `vfs_server` joins `KRN`) | `KRN` | P1 | — | S0 |
| [158](158-video-capture.md) | Video capture devices: the video gateway with consent, the camera mark and `camera` are done on a synthetic source; UVC cameras over `usb_host` (isochronous transfers) open; next: the MacBook Pro's FaceTime HD camera (EHCI, isochronous, UVC) | kernel + services | P1 | — | tracks A, G |
| [250](250-voice-dictation.md) | Voice V3, dictation: a Zipformer2 transducer, Russian first with `vosk-model-ru` 0.54 (5.2 % WER on FLEURS, 65M parameters), then English; a quality variant with GigaAM v3 (3.0 %) and Parakeet; models compared by measurement | main task, `APP` | P2 | — | track G |
| [251](251-model-cache-and-model-disk.md) | Speech models: a cache on the host with a manifest of SHA-256 (done in 251-APP-0009), a FAT32 model disk mounted as `models:`, models in the block store (251-STO-0010, done) | main task, `APP` | P2 | — (into memory: [251-STO-0015](251-STO-0015-a-model-read-into-memory.md)) | tracks G, B |
| [252](252-neural-speech-synthesis.md) | Neural speech synthesis: voices compared by intelligibility, naturalness and speed; eight chosen by ear (female and male, Russian and English, compact and quality) and in the model cache; the engines next | main task, `APP` | P2 | — | track G |
| [u015](u015-pins.md) | `pins`: the pins of an ARM board — list, every function of a pin with the active one marked, levels and changes through `gpio` (done except the board run) | tools | P2 | 205 | track H |
| [u017](u017-pins-view.md) | `pinmap`: the board's header on a screen, changes by keys after one confirmation; `pins` and `pinmap` from `wm` and `console` (done except the board run) | tools | P3 | 205 | track H |
| [205](205-aarch64-boards.md) | aarch64 on boards with UEFI: Raspberry Pi 4/5 (EDK2), servers with ACPI | porting | P2 | — (201–204 done) | track H |
| [207](207-gpio-service.md) | `gpio`: a user-space service for the pins of ARM boards (BCM2711, PL061); hwdocs pin tables | porting (done except hardware) | P2 | — (206 done) | track H |
| [210](210-apple-silicon-native.md) | Apple Silicon Macs natively (M1 first): boot through m1n1 and U-Boot, device tree, AIC, spin table, DART, DWC3 USB; moved from `PRT` to `APL`; **parked** 2026-10-09 (no Mac whose boot chain may be changed; the M5 at hand has no m1n1) | main task, `APL` (later) | P3 | an M1–M3 Mac whose boot chain may be changed; 205 | track H |
| [210-APL-0001](210-APL-0001-boot-through-m1n1-and-u-boot.md) | Boot through m1n1 and U-Boot: `BOOTAA64.EFI` from U-Boot's UEFI, entry at EL2; without ACPI no write to `virt`'s addresses | `APL` (open) | P3 | a Mac with M1 | track H |
| [210-APL-0002](210-APL-0002-board-from-the-device-tree.md) | The board from the device tree where there is no ACPI; host tests with the M1's trees, QEMU `virt,acpi=off` | `APL` (open) | P3 | — (210-KRN-0029 done) | track H |
| [210-APL-0003](210-APL-0003-aic-and-the-timer-fiq.md) | Apple's interrupt controller (AIC, AIC2) and the timer's FIQ: the tick, device lines, interrupts between CPUs | `APL` (open) | P3 | 210-APL-0002; a Mac with M1 | track H |
| [210-APL-0004](210-APL-0004-cpus-through-the-spin-table.md) | The other CPUs through the spin table instead of PSCI | `APL` (open) | P3 | 210-APL-0002, 0003; a Mac with M1 | track H |
| [210-APL-0005](210-APL-0005-reset-without-psci.md) | Reset through the watchdog, without PSCI; power off reported as unavailable | `APL` (open) | P3 | 210-APL-0002; a Mac with M1 | track H |
| [210-APL-0006](210-APL-0006-samsung-style-uart-console.md) | The console on the Samsung-style UART: the kernel's lines and `mind::dev::Uart` | `APL` (open) | P3 | 210-APL-0002, 0003; a Mac with M1 and its USB-C debug connection | track H |
| [210-APL-0007](210-APL-0007-dart-dma-boundary.md) | DART: each device's DMA only through its own IOMMU (MC-1.5); DMA regions and 16 KiB pages | `APL` (open) | P3 | 210-APL-0002; `KRN` (DMA regions); a Mac with M1 | track H |
| [210-APL-0008](210-APL-0008-usb-on-type-c-ports.md) | USB on the Type-C ports: DWC3 as an xHCI for `usb_host`, the ATC PHY, PMGR power | `APL` (open) | P3 | 210-APL-0003, 0007; a Mac with M1 | track H |
| [210-APL-0013](210-APL-0013-own-stage-two-instead-of-u-boot.md) | Our own second stage after m1n1, without U-Boot | porting, `APL` (open) | P3 | 210-APL-0001, a Mac | track H |
| [210-APL-0014](210-APL-0014-own-first-stage-instead-of-m1n1.md) | Our own first stage started by iBoot (`kmutil`), without m1n1 | porting, `APL` (open) | P3 | 210-APL-0013, a Mac | track H |
| [174](174-full-use-of-pc-hardware.md) | The PC's full capacity for programs: a complete hardware report each boot, every vector state component (AVX-512, AMX), performance states and deep idle, large pages, NUMA, the IOMMU, large BARs, then GPU and NPU computation (AMD, NVIDIA, Intel) behind one accelerator interface; aarch64 NEON/SVE | main task, `KRN` with `DRV`, `PRT` | P1 | — | tracks A, H |
| [174-KRN-0037](174-KRN-0037-every-vector-state-component.md) | Every XSAVE component for programs: AVX-512 and AMX, the area sized from CPUID; `FEATURE_AVX512`, `FEATURE_AMX` (in progress: done in QEMU, an AVX-512 machine left) | `KRN` | P1 | — | track H |
| [551](551-sound-on-pcs.md) | Sound on real PCs and the MacBook Pro: Intel HD Audio in `audio_gw` (speakers, headphones, microphone) so `listen`, `hear`, `voice` and `tts` work on real hardware; programs started by hand now, services started by hand later | main task, `DRV` | P1 | — | tracks A, G |
| [551-DRV-0010](551-DRV-0010-hda-controller-and-codecs.md) | HDA in `audio_gw`: controller, codec graph, output and input paths, playback and capture behind `idl/audio.wit`; QEMU `intel-hda` (in progress: done in QEMU, the MacBook Pro left) | `DRV` | P1 | — | track A |
| [173](173-boot-services-configuration.md) | Which boot services start and in what order: `data/services.txt` read by init, a safe start, `svc boot/enable/disable/after` (plan) | main task, `KRN` with `APP` | P2 | — | track A |
| [173-KRN-0035](173-KRN-0035-init-reads-the-service-configuration.md) | init reads and applies the service configuration; `boot-plan` in `init.wit` | `KRN` | P2 | — | track A |
| [173-KRN-0036](173-KRN-0036-safe-start.md) | A safe start: a key at the bootloader, a flag in `BootInfo`, the configuration ignored | `KRN`, with `PRT` | P2 | 173-KRN-0035 | track A |
| [211](211-intel-pc-from-a-sata-ssd.md) | An Intel PC booted from a SATA SSD (the maintainer's Samsung 860 PRO): the first real x86 machine | main task, `PRT` | P1 | — | track H |
| [211-PRT-0001](211-PRT-0001-writer-for-an-internal-disk.md) | The image writer for an internal SATA or NVMe disk, behind an explicit option | `PRT` | P2 | — | track H |
| [211-KRN-0044](211-KRN-0044-build-named-in-the-boot-log.md) | The boot log names the branch and commit built: `[INIT] BUILD: BRANCH …, COMMIT …` (made; the Mac's run left) | `KRN` | P1 | — | track A |
| [211-DRV-0018](211-DRV-0018-macbook-trackpad-gestures.md) | The MacBook Pro's trackpad in its multitouch mode: a right click with two fingers, scrolling with three (`usb.wit` 1.1 `reports-up-to`; made and host-tested, the Mac's run left) | `DRV` | P1 | — | track A |
| [211-KRN-0050](211-KRN-0050-a-program-on-an-unplugged-disk-is-refused-at-once.md) | A program started while its disk is unplugged is refused at once with the reason (`vfs_server` stops asking a silent drive, the journal backs off, `loader.wit` 1.7 `unreadable`; `usb_host`'s part requested from `DRV`) | `KRN` | P1 | — | track A |
| [211-KRN-0051](211-KRN-0051-setting-the-clock.md) | Setting the clock: `rtc.wit` 1.2 `set` on the CMOS RTC and the PL031 (the service done and host-tested), for the shell's `date set` (requested from `APP`) | `KRN` | P2 | — | track A |
| [351-KRN-0057](351-KRN-0057-the-download-check-on-utc-time.md) | The download check runs its machine on UTC time, so the certificates verify in any time zone (found on a host at UTC−6) | `KRN` | P1 | — | track C |
| [351-KRN-0052](351-KRN-0052-tpm-registers-from-the-firmware.md) | The TPM's registers from the firmware's tables: `PLATFORM_TPM` from the TPM2 table (x86) or the DSDT's `MSFT0101` (aarch64), for `tpm` and the sealed device key (made and host-tested; the `tls` suite's check left) | `KRN` | P1 | — | track C |
| [211-KRN-0053](211-KRN-0053-debug-mode-programs-log-to-the-log-volume.md) | Debug mode: with `log:debug.txt` the loader gives every program a log client, so what it prints reaches this boot's log file (made; the QEMU check left) | `KRN` | P1 | — | track A |
| [211-DRV-0017](211-DRV-0017-usb-input-delayed-by-hub-polling.md) | USB input a second late on the MacBook Pro: EHCI hub ports were polled by control transfers that each slept a tick (fixed, the Mac's run left) | `DRV` | P0 | — | track A |
| [211-DRV-0004](211-DRV-0004-ehci.md) | An EHCI driver for an Intel Mac's internal keyboard and trackpad (proposed) | `DRV` | P2 | — | track A |
| [211-DRV-0008](211-DRV-0008-usb-mouse-on-real-hardware.md) | A USB mouse on real hardware: the report protocol after the firmware's boot protocol; an interface whose setup fails is retried without flooding the log (in progress) | `DRV` | P2 | — | track A |
| [211-DRV-0019](211-DRV-0019-usb-storage-refusals-and-resets.md) | `usb_storage`: a refused command explained by REQUEST SENSE in the log, a lost device, its return and a reset logged; a refusal no longer taken for a lost device (from the MacBook Pro's hot-plug run) | `DRV` | P1 | [requests-KRN.md](requests-KRN.md) (a failed flush reported once) | track A |
| [211-DRV-0021](211-DRV-0021-a-gone-device-fails-at-once.md) | A request to a USB device that has gone fails at once: `usb_storage` claims a lost interface once per request (done); `usb_host`'s transfer waits and port checks after the kernel session's `usb_host` work reaches main | `DRV` | P1 | — | track A |
| [211-PRT-0006](211-PRT-0006-log-partition-in-the-image.md) | A 64 MiB FAT16 log partition `MIND LOG` in the disk images, which Windows, macOS and Linux mount (in progress) | `PRT` | P1 | — | track H |
| [211-KRN-0019](211-KRN-0019-boot-logs-on-the-log-partition.md) | `vfs_server` mounts the log partition as `log:` and writes each boot's system log there, `bootNNNN.log`, every 2 s (in progress) | `KRN` | P1 | — | track H |
| [000-KRN-0039](000-KRN-0039-kernel-kept-out-of-programs-pages.md) | The kernel kept out of programs' pages: SMEP, SMAP and UMIP on x86, PAN on aarch64, where the CPU has them; a boot line and the report say what was set (in progress: QEMU and the MacBook Pro's SMEP; a PC with SMAP left) | `KRN` | P1 | — | track K |
| [000-KRN-0020](000-KRN-0020-parallel-build.md) | The build runs its crates in parallel, one job per processor, each with its own log; failed ones are named with their errors (in progress) | `KRN` | P2 | — | track A |
| [212](212-disk-tools.md) | Disk tools on the target: list partitions, a service for raw disk changes, MBR/GPT, FAT formatting, mounting (plan) | main task, `PRT` with `KRN`, `APP` | P3 | — (211-KRN-0012 done) | track H |
| [211-DRV-0002](211-DRV-0002-ahci-every-port.md) | `ahci`: every port with a disk and every controller | `DRV` | P2 | — (211-KRN-0012 done) | track A |
| [211-PRT-0004](211-PRT-0004-first-run-on-an-intel-pc.md) | The first run on the maintainer's Intel PC from the 860 PRO, recorded as profile `x86-64/PC-0` | `PRT`, with the maintainer | P1 | the PC (issues-human 5) | track H |
| [300-STO-0011](300-STO-0011-a-client-that-may-only-store.md) | A client that may only store, refused a read on the platform (split from 300-STO-0004) | `STO` | P3 | a put-only client, when a program needs one | track B |
| [350](350-signed-boot-images.md) | Signed boot images and a launch record (track C, first step): manifest, signing, verification in the bootloader and the reproducible-build check done; the launch record readable in the system remains (`350-UPD-0004`) | main task, `UPD` | P2 | — | track C |
| [350-UPD-0004](350-UPD-0004-launch-record.md) | The launch record: printed on the serial line at every verified boot; readable in the system once the kernel keeps it | `UPD` | P2 | [requests-KRN.md](requests-KRN.md) | track C |
| [351](351-self-update.md) | Self-update: fetch over HTTPS or SSH, verify, stage in slot A or B, activate with last-known-good | main task, `UPD` | P1 | 350 | track C |
| [351-UPD-0007](351-UPD-0007-updater-service.md) | The `updater` service: check, fetch, verify, stage, apply, roll back (decomposed; 0013 and 351-NET-0011 done) | `UPD` | P1 | — (0008 and 351-KRN-0022 done) | track C |
| [351-UPD-0009](351-UPD-0009-rollback-policy-and-key-roles.md) | Minimum version, expiry, key roles, rotation, compromise protocol | `UPD` | P2 | 351-UPD-0007 | track C |
| [351-UPD-0010](351-UPD-0010-updating-the-bootloader.md) | Updating the bootloader itself (two loaders, `BootNext`) | `UPD`, with `PRT` | P3 | — (351-KRN-0022 and 351-KRN-0027 done) | track C |
| [351-UPD-0011](351-UPD-0011-version-floor-in-the-tpm.md) | A version floor in a TPM 2.0 counter, kept by the bootloader (rollback with the disk in hand) | `UPD`, with `PRT` | P2 | 351-UPD-0009, 0012 | track C |
| [351-UPD-0012](351-UPD-0012-secure-boot-with-our-own-keys.md) | Secure Boot with our own keys; old bootloaders revoked through dbx: done and tested in QEMU (OVMF), the run on a real PC remains | `UPD`, with `PRT` | P2 | the maintainer's PC (issues-human 5) | track C |
| [251-STO-0015](251-STO-0015-a-model-read-into-memory.md) | A model read back into memory for a recognizer: a memory object of a file from the store, or a copy made once (split from 251-STO-0010) | `STO` | P3 | — (the store's memory quota: requests-KRN.md) | track B |
| [351-STO-0006](351-STO-0006-releases-pinned-in-the-store.md) | Releases as objects in the block store, the running and last-known-good ones pinned by the updater (MC-9.3) | `STO` | P3 | 351-UPD-0007 (the store's disk: 300-KRN-0025, done) | track B, C |
| [109-NET-0010](109-NET-0010-a-parser-per-session.md) | A parser process per session, started with no client and ended with its session (B.6) | `NET` | P3 | a kernel and loader change (a spawn with no standard clients) | track D |
| [351-NET-0004](351-NET-0004-ssh-client.md) | An SSH client: curve25519, ssh-ed25519, a pinned host key, SFTP reads | `NET` | P3 | — | track D |
| [351-NET-0006](351-NET-0006-device-key-sealed-by-a-tpm.md) | The device key sealed by a TPM instead of stored unencrypted: built in `keystore`, checked with a sketch of the kernel's half | `NET` | P2 | 351-DRV-0015 (requests-KRN: the TPM's registers) | track D |
| [351-DRV-0015](351-DRV-0015-tpm-driver.md) | A TPM 2.0 driver: the `tpm` service (CRB, FIFO), seal and unseal for `keystore` built and host-tested; the NV counter with 351-UPD-0011 | `DRV` | P2 | [requests-KRN.md](requests-KRN.md) (the TPM's registers, `PLATFORM_TPM`) | track A |
| [351-ASR-0005](351-ASR-0005-power-loss-during-update.md) | Power loss at every step of an update, in QEMU | `ASR` | P1 | 351-UPD-0007 (351-KRN-0014 done) | Assurance |
| [351-ASR-0006](351-ASR-0006-update-threat-model.md) | The update threat model; fuzzing the metadata parsers — the model and three fuzzers written (on a branch), two findings sent to `UPD` | `ASR` | P2 | 351-UPD-0005 | Assurance |
| [400](400-marain-m0-m2-host-bench.md) | Marain M0–M2 on a host bench (track E, first step) | main task, `MRN` (open) | P3 | — | track E |
| [500](500-fuzzing-abi-and-idl.md) | Fuzzing the system calls and the IDL decoders (Assurance, first step) | main task, `ASR` | P2 | — | Assurance |
| [500-ASR-0001](500-ASR-0001-idl-decoder-fuzzing.md) | Every generated IDL decoder fuzzed on the host: 24 receivers and 82 types, a fixed seed, 50 000 inputs each; no finding so far; in CI once the kernel track adds the line | `ASR` | P2 | [requests-KRN.md](requests-KRN.md) (the CI line) | Assurance |
| [501](501-effector.md) | The effector: an agent of the Effector server on the target, so updates and tests run on real hardware unattended; results recorded per configuration; new images only through self-update (351) | main task, `ASR` | P2 | 550; Effector's agent protocol (the maintainer); `init`'s grants (`KRN`) | Assurance |
| [550](550-network-on-real-hardware.md) | Network on real hardware: the MacBook Pro over a USB Ethernet adapter first, then Wi-Fi and PCs | main task, `DRV` | P2 | — | tracks A, D |
| [550-DRV-0005](550-DRV-0005-usb-ethernet.md) | A USB Ethernet class driver (CDC-ECM, CDC-NCM) over `usb_host`; the choice of configuration and alternate setting in `usb_host` | `DRV` | P2 | — (the check with `netstack`: [requests-NET.md](requests-NET.md), `init`) | track A |
| [550-DRV-0006](550-DRV-0006-broadcom-wifi.md) | The MacBook Pro's Broadcom Wi-Fi (BCM4331 expected, SoftMAC): the PCI ID and the firmware licence first, then the driver (proposed) — the chip confirmed as BCM4331 and the firmware decided (2026-10-09); stage 1 is 550-DRV-0020 | `DRV` | P1 | the PCI ID; the firmware licence; `NET`'s 802.11 station | track A |
| [550-DRV-0022](550-DRV-0022-bcm4331-core-reset-and-sprom.md) | `bcm_wifi` stage 1b: the 802.11 core the firmware left running held in reset; the SPROM read with its pins taken from the amplifier lines; written, waits for a run on the MacBook Pro | `DRV` | P1 | — | track A |
| [550-DRV-0007](550-DRV-0007-broadcom-ethernet.md) | Broadcom tg3-family Ethernet: Apple's Thunderbolt Gigabit Ethernet adapter and PCs; no QEMU model (proposed) | `DRV` | P3 | hardware with such a chip | track A |
| [700](700-bluetooth.md) | Bluetooth: the HCI transport over `usb_host`, the host stack, pairing, HID, A2DP and other devices, the `bt` tool | main task, `BLT` (later: after Wi-Fi) | P3 | — | the maintainer's request |
| [600](600-apple-silicon-mac-vm-host.md) | An Apple Silicon Mac as a host: the aarch64 system in a virtual machine with HVF, built, run and tested on macOS ([guide](../docs/apple-silicon.md)); **parked** 2026-10-09: the goal is bare metal, not a virtual machine | main task, `APL` (later) | P3 | a person with an Apple Silicon Mac | track H |
| [600-APL-0009](600-APL-0009-aarch64-build-on-macos.md) | The aarch64 build with the Bash 3.2 and BSD sed macOS ships (`mapfile`, a GNU sed form) | `APL` (open) | P2 | — (the check: a Mac) | track H |
| [600-APL-0010](600-APL-0010-run-script-on-macos.md) | `03_run_qemu_aarch64.sh` on macOS: the screen in a window, Bash 3.2, `MIND_ACCEL`, memory above 3 GiB on an M1 | `APL` (open) | P2 | — (the check: a Mac) | track H |
| [600-APL-0011](600-APL-0011-first-run-on-a-mac.md) | The first run on an Apple Silicon Mac under HVF, by hand: RNDR, device accesses, ITS, CPUs, memory; recorded in the guide and the profile | `APL` (open) | P2 | a person with an Apple Silicon Mac | track H |
| [600-APL-0012](600-APL-0012-aarch64-suites-on-a-mac.md) | The aarch64 suites on a Mac under TCG and HVF (accelerator, firmware, no `/proc`); evidence for the HVF configuration | `APL` (open) | P3 | a person with a Mac; the test harness (`KRN`) | track H |
| [650](650-building-on-the-target.md) | Building on the target: a read-only git client, builds through a server, then a POSIX layer, Rust and self-hosting (long-term) | main task, `DEV` (proposed) | P3 | the track's confirmation; by stage: 501, `NET`'s SSH client, `KRN` features | — (proposed track) |

Requests that wait for a track to number them: [requests-UPD.md](requests-UPD.md) (`release.check` raising on bad signed channels; the trial's count-down at the largest sequence number, for 351-ASR-0006); [requests-KRN.md](requests-KRN.md) (the IDL fuzzer in CI's host tests; the toolchain installed before the parallel build; `bcm_wifi` as a boot service); [requests-APP.md](requests-APP.md) (full screen and a list of windows in `wm`; clocks asking the RTC service 10 times a second; `update` in the shell for 351; a Wi-Fi setup program for 550; `log:` in the shell's help and in `fm`; `efivar` in the tools guide; the shell lending its TLS client; `svc` for the boot services, for 173); [requests-NET.md](requests-NET.md) (several network interfaces, an 802.11 station with WPA2-PSK, for 550).


## Finished tasks (`issues-done/`)

| № | Task | Result |
|---|---|---|
| [001](../issues-done/001-flat-binary-entry-offset-and-got-call.done) | Flat binaries: `_start` offset and GOT call | superseded by 002 (2026-10-03) |
| [002](../issues-done/002-elf-loader.done) | ELF loader instead of flat binaries | done (2026-10-03) |
| [003](../issues-done/003-bss-and-heap-allocator.done) | `.bss`, statics, heap | done (2026-10-03) |
| [004](../issues-done/004-apic-idt-interrupts.done) | IDT + APIC, interrupt-driven timer and keyboard | done (2026-10-03) |
| [005](../issues-done/005-syscalls.done) | `int 0x80` system calls, shared ABI | done (2026-10-03) |
| [006](../issues-done/006-bootloader-load-from-fat32.done) | Bootloader reads images from FAT | done (2026-10-03) |
| [007](../issues-done/007-kernel-font-and-primitives.done) | Font, primitives, console | done; panic diagnostics split into 018 (2026-10-03) |
| [008](../issues-done/008-kernel-timeout-handoff.done) | Handoff to userspace on timeout | superseded (2026-10-03) |
| [009](../issues-done/009-gop-pixel-format.done) | Honour `PixelFormat` and select a GOP mode | done (2026-10-04) |
| [010](../issues-done/010-docs-sync.done) | Sync README/handoff with the code | done (2026-10-03) |
| [011](../issues-done/011-reproducible-toolchain.done) | Reproducible build: pinned toolchain, lock files, CI | done (2026-10-04) |
| [012](../issues-done/012-multitasking-and-program-instances.done) | Multitasking, instances, `ps`/`kill`/`fg` | done (2026-09-18) |
| [013](../issues-done/013-smp-and-memory-isolation.done) | SMP, ring 3, memory isolation | done (2026-09-19) |
| [014](../issues-done/014-private-program-heap.done) | Private program heap | done (2026-09-19) |
| [015](../issues-done/015-load-programs-through-vfs.done) | Loading programs through the VFS | done (2026-10-03) |
| [016](../issues-done/016-storage-drivers.done) | ATA / AHCI / USB storage drivers | done (2026-10-03) |
| [017](../issues-done/017-tts-on-audio-gateway.done) | Text to speech over the audio gateway | done (2026-10-03) |
| [018](../issues-done/018-kernel-panic-diagnostics.done) | Kernel panic prints message, location, CPU and task | done (2026-10-04) |
| [019](../issues-done/019-platform-profile-x86-64-qemu-0.done) | Platform profile `x86-64/QEMU-0` | done (2026-10-03) |
| [020](../issues-done/020-init-bootstrap-authority.done) | `init`: bootstrap authority, driver policy out of the kernel | done (2026-10-03) |
| [021](../issues-done/021-shell-in-ring-3.done) | Shell in ring 3, input/focus policy out of the kernel | done (2026-10-03) |
| [022](../issues-done/022-audio-tools-say-listen.done) | Audio tools `say <text>`, `listen`; program arguments; AC97 capture | done (2026-10-03) |
| [023](../issues-done/023-monotonic-clock.done) | Monotonic clock with defined resolution | done (2026-10-03) |
| [024](../issues-done/024-accounted-kernel-objects.done) | Per-owner quotas for tasks and endpoints | done (2026-10-03) |
| [025](../issues-done/025-capability-generations.done) | Capability handles with generations | done (2026-10-03) |
| [026](../issues-done/026-derivation-and-revocation.done) | Capability derivation, attenuation and revocation | done (2026-10-03) |
| [027](../issues-done/027-no-ambient-endpoint-names.done) | No ambient endpoint names | done (2026-10-03) |
| [028](../issues-done/028-memory-rights-and-lease.done) | Memory rights, read-only mappings, leases ended by revoke | done (2026-10-03) |
| [029](../issues-done/029-memory-objects-move-seal.done) | Memory objects: MOVE and sealed SHARE_RO | done (2026-10-04) |
| [030](../issues-done/030-ipc-bounds-and-cancellation.done) | IPC bounds, timeouts and cancellation | done (2026-10-04) |
| [031](../issues-done/031-mind-idl-v0.done) | MIND IDL v0: WIT subset, generated bindings, receiver checks (`rtc`) | done (2026-10-04) |
| [032](../issues-done/032-minimal-supervision.done) | Minimal supervision: exit notices, restart budget, quarantine, fencing | done (2026-10-04) |
| [033](../issues-done/033-audit-and-supervision-follow-ups.done) | Follow-ups: multi-process tests, device stop, platform privilege dropped, quotas | done (2026-10-04) |
| [034](../issues-done/034-key-events-input-queue.done) | Key events in the per-task input queue | done (2026-10-04) |
| [035](../issues-done/035-observe-and-stat.done) | OBSERVE privilege, `STAT`, firmware memory map | done (2026-10-04) |
| [036](../issues-done/036-endpoint-badges.done) | Endpoint badges | done (2026-10-04) |
| [037](../issues-done/037-task-limit.done) | Task limit 32, 127 endpoints | done (2026-10-04) |
| [038](../issues-done/038-scheduling-budgets.done) | Scheduling budgets and bands (C7) | done (2026-10-04) |
| [039](../issues-done/039-port-services.done) | Port the services to MIND IDL and the C4 memory modes (C8) | done (2026-10-04) |
| [040](../issues-done/040-program-heap.done) | Program heap `mind::alloc` | superseded by 052 (2026-10-04) |
| [041](../issues-done/041-font-8x16.done) | 8×16 font with Cyrillic and box drawing | superseded by 053 (2026-10-04) |
| [042](../issues-done/042-tui-library.done) | TUI library `mind::tui` | superseded by 054 (2026-10-04) |
| [043](../issues-done/043-keyboard-decoding-and-line-editing.done) | Keyboard decoding, layouts, VT100 input, shell line editing | superseded by 055, 056 (2026-10-04) |
| [044](../issues-done/044-idl-v02-records-strings.done) | MIND IDL v0.2: records, strings, lists in buffers; `loader.wit` | done (2026-10-04) |
| [045](../issues-done/045-sysmon-and-monitors.done) | `sysmon` and `top`, `memmap`, `load`, `hw` | superseded by 060, 061 (2026-10-04) |
| [046](../issues-done/046-loader-sessions.done) | Loader v1: launch sessions with granted capabilities | superseded by 062 (2026-10-04) |
| [047](../issues-done/047-viewer-and-fm-readonly.done) | Viewer `view`, file manager `fm` read-only | superseded by 057, 063 (2026-10-04) |
| [048](../issues-done/048-write-path-ramdisk-vfs2.done) | Block write, `ramdisk`, VFS v2 with directory handles | superseded by 064–066 (2026-10-04) |
| [049](../issues-done/049-editor-and-fm-write.done) | Editor `edit`, `fm` write operations, `df`, `fsck` | superseded by 067, 068 (2026-10-04) |
| [050](../issues-done/050-logd-dmesg-svc.done) | `logd`, `dmesg`, `svc` | superseded by 069, 070 (2026-10-04) |
| [051](../issues-done/051-merge-main-into-tools.done) | Merge `main` into the tools branch, reconcile duplicate IDL/STAT/input/badges and issue numbers | done (2026-10-04) |
| [052](../issues-done/052-program-heap.done) | Program heap `mind::alloc` | done (2026-10-04) |
| [053](../issues-done/053-font-8x16.done) | 8×16 font with Cyrillic and box drawing | done (2026-10-04) |
| [054](../issues-done/054-tui-library.done) | Text UI library `mind::tui` | done (2026-10-04) |
| [055](../issues-done/055-key-events.done) | Key events: E0 keys, modifiers, layouts, VT100 | done (2026-10-04); on the event words of 034 after the merge |
| [056](../issues-done/056-shell-line-editing.done) | Shell: line editing, history, Cyrillic | done (2026-10-04) |
| [057](../issues-done/057-viewer.done) | Viewer `view` | done (2026-10-04) |
| [058](../issues-done/058-mind-idl-v0.2.done) | MIND IDL v0.2: records, strings, lists | done (2026-10-04); after the merge 044 is the base, enums, `bytes<N>` and capability results are its minor extension |
| [059](../issues-done/059-observation-abi.done) | OBSERVE privilege, `STAT`, firmware memory map | done (2026-10-04); after the merge the `STAT` of 035 is used, lost fields in 075 |
| [060](../issues-done/060-sysmon.done) | `sysmon` service | done (2026-10-04) |
| [061](../issues-done/061-top-memmap-load-hw.done) | `top`, `memmap`, `load`, `hw` | `monitor/`: `top`, `memmap`, `load`, `hw` on sysmon; `Key::latin` |
| [062](../issues-done/062-loader-v1-launch-grants.done) | Loader v1: launch with granted capabilities | launch sessions (`idl/loader.wit` 1.1), `mind::request!`, console programs, `uptime` program |
| [063](../issues-done/063-file-manager-read-only.done) | File manager `fm`, read-only | `fm`: two panels, viewer, quick view, info, find, run; VFS LIST with attributes and times |
| [064](../issues-done/064-endpoint-badges-block-write.done) | Endpoint badges and block write | BLOCK_WRITE/FLUSH for the write badge (`idl/block.wit` 1.1 after the merge, badges of 036); ATA/AHCI/USB write; `block` suite |
| [065](../issues-done/065-ramdisk.done) | `ramdisk` block service | 8 MiB RAM disk service, formatted FAT16 and mounted as ram: |
| [066](../issues-done/066-vfs-v2-fat-write.done) | `vfs_server` v2: directory handles, FAT write | vfs.wit 2.x with handles and zones; FAT12/16/32 writer with long names; write-back cache; shell file commands |
| [067](../issues-done/067-editor.done) | Editor `edit` | edit: piece table with undo, search/replace, menu and dialogs; saves via name.tmp; REQUEST_FILE lends the user's VFS client |
| [068](../issues-done/068-fm-write-df-fsck.done) | `fm` write operations, `df`, `fsck` | fm: copy/move/mkdir/delete jobs on A: and ram:, built-in editor; df; fsck (vfs.wit 2.1 check) |
| [069](../issues-done/069-logd-dmesg.done) | `logd` and `dmesg` | logd: ring with stamped sources, rate limit, read badge; println lines of services go there; dmesg; logger |
| [070](../issues-done/070-svc-lifecycle.done) | `svc` and lifecycle control | lifecycle requests served by init (`idl/init.wit` 1.1 after the merge); svc; top stops and restarts |
| [071](../issues-done/071-scoped-file-grants.done) | Scoped file grants for launched programs | vfs.wit scope: the editor's client is confined to its file's directory and revoked on exit; REQUEST_FILES for fm |
| [072](../issues-done/072-fixed-grant-slots.done) | Fixed capability slots for launcher grants (`SLOT_DYNAMIC` 16) | done (2026-10-04) |
| [073](../issues-done/073-port-out-block.done) | `PORT_OUT_BLOCK`: block writes of 16-bit words to a port | done (2026-10-04) |
| [074](../issues-done/074-exited-console-output.done) | Output of an exited console program stays readable | done (2026-10-04) |
| [075](../issues-done/075-stat-fields-for-the-monitors.done) | `STAT` fields the monitors lost in the merge (`STAT_VERSION` 2) | done (2026-10-04) |
| [076](../issues-done/076-monitors-show-restored-stat-fields.done) | The monitors show the restored `STAT` fields (`sysinfo.wit` 2.0) | done (2026-10-04) |
| [077](../issues-done/077-voice-audio-front-end.done) | Voice V0: audio front end (`mind::voice`, `listen --vad/--wav`) | done (2026-10-04) |
| [078](../issues-done/078-voice-command-recognizer.done) | Voice V1: offline command recognizer (`hear`, `mind::voice` model and grammar) | done (2026-10-04) |
| [079](../issues-done/079-voice-control-in-the-shell.done) | Voice V2: voice control in the shell (`voice`, `idl/voice.wit`, confirmations, spoken replies, `audio.wit` 1.1) | done (2026-10-04) |
| [080](../issues-done/080-ipc-tool.done) | `ipc`: endpoints, holders, wait-for graph | done (2026-10-04) |
| [081](../issues-done/081-caps-tool.done) | `caps`: capabilities and the derivation tree (`sysinfo.wit` 3.0 authority) | done (2026-10-04) |
| [082](../issues-done/082-find-and-grep.done) | `find` and `grep` | done (2026-10-04) |
| [083](../issues-done/083-format.done) | `format` for the RAM disk (`vfs.wit` 2.3) | done (2026-10-04) |
| [084](../issues-done/084-reboot.done) | `reboot [-f]`: flush, services stopped in reverse order, reset | done (2026-10-04) |
| [085](../issues-done/085-keymap.done) | `keymap`: layout and switch key (`keyboard.wit` 1.0) | done (2026-10-04) |
| [086](../issues-done/086-screenshot.done) | `screenshot`: the screen as a BMP (`display.wit` 1.0) | done (2026-10-04) |
| [087](../issues-done/087-tts-idle-tone.done) | `tts`: a quiet tone stayed after every phrase (fixed-point limit cycle; filters cleared after 30 ms without excitation) | done (2026-10-04) |
| [088](../issues-done/088-text-window-manager.done) | `wm`: window manager — text and pixel programs in windows (keys and mouse, snapping, detach keeps them running); programs open windows through `mind::windowed` | done (2026-10-05) |
| [089](../issues-done/089-text-clock-faces.done) | Text faces for `clock` and `dzen-clock` (`--text`: large digits, colored cells; on a screen or in a `wm` text window) | done (2026-10-05) |
| [090](../issues-done/090-read-only-status-and-modifier-key-bars.done) | Editor says READ-ONLY; key bars follow Shift, Ctrl and Alt | done (2026-10-04) |
| [091](../issues-done/091-program-list-fits-the-screen.done) | `list`: sorted in columns that fit the screen; `list -l` says what each program does | done (2026-10-04) |
| [092](../issues-done/092-help-for-every-program.done) | `help <program>`, and `--help` in every application (`mind::about!`) | done (2026-10-04) |
| [093](../issues-done/093-screen-recording.done) | `record`: the screen as AVI/MJPEG (`mind::jpeg` with a restart per row of blocks, `mind::avi`); one window: u014 | done (2026-10-06) |
| [094](../issues-done/094-shell-script-language.done) | `msh`: the shell's script language — values, records, results as in Marain (`?`, `or`, `try`), commands as typed, `capture`/`ps()`/`files()`, `requires:` limits a script's authority | done (2026-10-06) |
| [095](../issues-done/095-list-by-mask.done) | `list a*`: the programs whose names match a mask (`mind::mask` without allocation) | done (2026-10-04) |
| [096](../issues-done/096-audio-without-interrupts.done) | `say` and `listen` hung when the sound card shared its interrupt line (`audio_gw` looks at its ring while a client waits) | done (2026-10-05) |
| [097](../issues-done/097-fm-command-line-and-hidden-panels.done) | fm: the command line under the panels (`cd`, `edit`, `view`, programs with arguments) and hiding panels with Ctrl+O, Ctrl+F1/F2, Ctrl+P | done (2026-10-05) |
| [098](../issues-done/098-pong-shows-the-exchange.done) | `pong` kept the string it read on screen for 10 ms only; it stays now, with the number of calls, and Esc works between calls | done (2026-10-05) |
| [099](../issues-done/099-fm-starts-programs-in-windows.done) | fm in a window of `wm` starts programs in windows of their own (it lends its broker client, files and system information) | done (2026-10-05) |
| [u001](../issues-done/u001-mouse-in-windows-and-fm.done) | The mouse inside windows (`wm` passes clicks, drags and the wheel to the program at the cell of its content) and in fm (click, double click, right click, wheel, key bar; the pointer cell on a screen) | done (2026-10-05) |
| [u002](../issues-done/u002-restore-and-unsnap-windows.done) | wm: `[▲]` maximizes, `[⇕]` gives a maximized or snapped window its frame back; dragging a snapped title off the edge does too | done (2026-10-05) |
| [u003](../issues-done/u003-desktop-programs-menu.done) | wm: a right click on the desktop (or Alt+P) opens the programs by category; a click starts one in a window | done (2026-10-05) |
| [u004](../issues-done/u004-console.done) | `console`: a terminal for programs in a window or on a screen; console programs started in `wm` or `fm` run in it | done (2026-10-05) |
| [u005](../issues-done/u005-beep-without-a-screen.done) | `beep` without a screen: `beep 440` sounds 500 ms, `beep 440 200 0 100 880 300` a series (0 Hz: a pause) | done (2026-10-05) |
| [u006](../issues-done/u006-console-commands.done) | console: its own `ps`, `ls`, `cat`, `date`, `time`, `ping` (through its policy grant); the shell's commands named; `run <program>` | done (2026-10-05) |
| [u007](../issues-done/u007-time-clock-dzen-text.done) | The shell's one-line `clock` is `time`, `clock` starts the clock again; dzen-clock: T switches to the text face | done (2026-10-05) |
| [u008](../issues-done/u008-clickable-top-bar.done) | wm: the items of the top bar can be clicked instead of their keys (for a host that keeps Alt+Tab for itself) | done (2026-10-05) |
| [u009](../issues-done/u009-pixel-windows-follow-their-frame.done) | wm: a pixel window's content follows its frame (`clock` and `dzen-clock` laid out again at its size) | done (2026-10-05) |
| [u010](../issues-done/u010-say-text-on-screen.done) | `say` shows its text whole on its screen, Cyrillic as it is (8x16 font, rows cut at spaces) | done (2026-10-05) |
| [u011](../issues-done/u011-beep-from-the-desktop-menu.done) | beep from wm's desktop menu: works (its lines in console, its tones in a WAV), now tested; console says when every program ends | done (2026-10-06) |
| [u012](../issues-done/u012-load-graphs-aligned.done) | `load`: every graph ends at the same column and the scale labels end in one column, whatever their widths | done (2026-10-06) |
| [u013](../issues-done/u013-quit-from-menus-and-key-bars.done) | Quit in a program's menu and key bar: `edit` and `view` take the mouse, menus take clicks, F10 quits from edit's open menu | done (2026-10-06) |
| [u014](../issues-done/u014-record-a-window.done) | `record -w` in wm's run line records the window in front alone: wm lends a read-only lease of its surface and marks its frame " ● REC " | done (2026-10-06) |
| [u016](../issues-done/u016-clock-console-faces.done) | `clock --line`, `dzen-clock --line`: console programs whose line is written again with `\r`; `REQUEST_LINE` and `mind::process::console_run` | done (2026-10-06) |
| [170-APP-0001](../issues-done/170-APP-0001-escrow-in-caps.done) | `caps` and `top` name the escrow capability kind (issue 170): `escrow ----  of control` | done (2026-10-06) |
| [000-APP-0004](../issues-done/000-APP-0004-pinmap-check-whole-line.done) | The `pinmap` check waits for the whole `[PINMAP] READY` line (the storage track's request) | done (2026-10-06) |
| [000-APP-0003](../issues-done/000-APP-0003-svc-restart-loader-race.done) | `svc restart loader` could lose its own start: svc first makes one call to the loader, which then has answered the shell | done (2026-10-06) |
| [171-APP-0002](../issues-done/171-APP-0002-sysinfo-pages.done) | `sysinfo.wit` 4.0: tasks and endpoints page by page; `sysmon`, `top`, the console's `ps`, `logd` see every task (171) | done (2026-10-06) |
| [171-APP-0006](../issues-done/171-APP-0006-monitor-bounds.done) | `top`, `memmap`, `load`: capabilities as `n/4095`, counts of tasks and endpoints without the root quota, the task graph to its own scale, the EP column fits 65 535 (requested by `KRN`) | done (2026-10-07) |
| [171-APP-0007](../issues-done/171-APP-0007-sysinfo-every-cpu-and-capability.done) | `sysinfo`: every CPU and every capability in pages; `top` and `load` show 16 CPUs and more (requested by `KRN`) | done (2026-10-07) |
| [171-APP-0008](../issues-done/171-APP-0008-memory-check-every-cpu-count.done) | `applications_until_memory_ends` with every CPU count: the groups "16 CPUs" fill memory too (78 clocks on x86, 171 on aarch64) | done (2026-10-08) |
| [158-APP-0005](../issues-done/158-APP-0005-camera-mark-on-ci.done) | The camera mark was missing on CI: `video_gw` could sleep a minute (fixed in 158-DRV-0001); the check reports a stall with `ps` and `stat` | done (2026-10-07) |
| [158-DRV-0001](../issues-done/158-DRV-0001-video-gw-frame-wait.done) | `video_gw`: a frame wait read the clock twice and could sleep 60 s (done by `APP` for 158-APP-0005, open track) | done (2026-10-07) |
| [251-APP-0009](../issues-done/251-APP-0009-model-cache-on-the-host.done) | The model cache on the host: `models/manifest.toml`, `scripts/models.py` (fetch, verify, pack, disk), `scripts/fat32.py` | done (2026-10-08) |
| [251-APP-0010](../issues-done/251-APP-0010-models-volume.done) | `models:` in the system: `vfs_server` mounts the volume labelled MIND MODELS read-only; `df`, `fsck`, a new `sha256`; `MIND_MODELS_DISK` in `03_run_qemu.sh` (x86) | done (2026-10-08) |
| [252-APP-0011](../issues-done/252-APP-0011-chosen-voices-in-the-cache.done) | The eight voices the maintainer chose in the model cache (Vosk TTS 0.7 and 0.9, ESpeech with its vocoder, stress models and reference, Piper lessac and ryan, Kokoro bf_emma); `models.py`: variant lists, voices, needs, tar archives, `pin` by directory | done (2026-10-08) |
| [100](../issues-done/100-virtio-net-driver.done) | `virtio_net`: network card driver in ring 3 | done (2026-10-04) |
| [101](../issues-done/101-network-stack.done) | Network stack `netstack` (DHCP, ICMP, DNS, UDP, TCP) | done (2026-10-04) |
| [104](../issues-done/104-virtio-modern-msix.done) | Modern VirtIO interface and MSI-X interrupts | done (2026-10-04) |
| [102](../issues-done/102-network-policy-broker.done) | Network policy broker and flow grants | done (2026-10-04) |
| [103](../issues-done/103-tls-service.done) | TLS service with non-exportable keys | done (2026-10-04) |
| [105](../issues-done/105-multiple-network-cards.done) | Several network cards: driver instances per card, stack interfaces | done (2026-10-04) |
| [106](../issues-done/106-network-offloads.done) | Checksum and segmentation offloads, after measurement (transmit checksum offload, off by default) | done (2026-10-04) |
| [107](../issues-done/107-batched-frame-path.done) | Batched frame path between the stack and the card drivers (frame ring) | done (2026-10-04) |
| [150](../issues-done/150-user-memory-beyond-the-arena.done) | User memory beyond the kernel arena: frame pool, memory quotas delegated at `SPAWN`, large sealed objects | done (2026-10-05) |
| [153](../issues-done/153-xsave-avx-state.done) | XSAVE: AVX state per task | done (2026-10-05) |
| [154](../issues-done/154-push-to-talk-routing.done) | Push-to-talk routing: keys taken from the focused program for a listener (`INPUT_LISTEN`) | done (2026-10-05) |
| [156](../issues-done/156-ps2-mouse.done) | PS/2 mouse: pointer events for the focused program | done (2026-10-04) |
| [157](../issues-done/157-window-broker.done) | `windows`: window broker, windows that outlive the window manager | done (2026-10-04) |
| [159](../issues-done/159-shared-interrupt-lines.done) | Shared interrupt lines reach every driver on them | done (2026-10-05) |
| [160](../issues-done/160-absolute-pointer-tablet.done) | Absolute pointer: a VirtIO tablet, no pointer grab in the emulator (the porting stream's; the same work as 161, merged into it; 160-focus-for-a-started-program is another issue) | merged into 161 (2026-10-05) |
| [200](../issues-done/200-architecture-layer.done) | Architecture layer in the kernel and libmind (x86-64 first) | done (2026-10-05) |
| [201](../issues-done/201-aarch64-boot.done) | aarch64 on QEMU `virt`: boot to init | done (2026-10-05) |
| [202](../issues-done/202-aarch64-devices.done) | aarch64 devices: PCIe ECAM and the ITS, VirtIO block/net/input, PL011, PL031, display | done (2026-10-05) |
| [203](../issues-done/203-aarch64-smp-and-power.done) | aarch64 SMP: CPUs from the MADT started through PSCI, SGIs, reset and power off | done (2026-10-05) |
| [204](../issues-done/204-aarch64-profile-and-ci.done) | aarch64 profile `aarch64/QEMU-virt-0` and CI: `ARCH=aarch64 ./02_build.sh`, three CI groups | done (2026-10-05) |
| [206](../issues-done/206-pin-controllers-from-firmware.done) | aarch64: pin controllers (BCM2711 GPIO, PL061) from the DSDT and SSDTs, their registers by `PLATFORM_MMIO` index | done (2026-10-06) |
| [208](../issues-done/208-aarch64-idle-check-margin.done) | aarch64 idle check: the lowest of three samples against 0.85 s; passes under load, fails when idle CPUs spin (3.85 s) | done (2026-10-06) |
| [151](../issues-done/151-shell-grant-slots-13-15.done) | Shell grant slots 13–15: authority view, keyboard, display | done (2026-10-04) |
| [152](../issues-done/152-reboot-system-call.done) | `REBOOT` system call | done (2026-10-04) |
| [161](../issues-done/161-absolute-pointer-virtio-tablet.done) | Absolute pointer events and the `virtio_input` driver: with QEMU's VirtIO tablet the system's pointer follows the host's to every edge | done (2026-10-05) |
| [162](../issues-done/162-console-output-slot.done) | `SLOT_CONSOLE`: a launcher may lend an endpoint where what the program prints goes too (`mind::output`) | done (2026-10-05) |
| [163](../issues-done/163-window-broker-memory.done) | init gives the window broker a 128 MiB memory quota: pixel windows with room for the screen | done (2026-10-05) |
| [165](../issues-done/165-display-client-for-programs.done) | A launcher may lend the compositor client (`REQUEST_DISPLAY`); the compositor shows a red dot while the screen is captured | done (2026-10-06) |
| [164](../issues-done/164-usb-hid-keyboard-and-mouse.done) | USB keyboards, mice and tablets: `usb_host` (xHCI, hubs, hot plug, `idl/usb.wit`), `usb_hid`, `usb_storage` over it; no `ps2_kbd` without a controller; Intel chipsets' ports moved from EHCI | done (2026-10-06) |
| [155](../issues-done/155-virtual-consoles.done) | Virtual consoles: the shell's four consoles, Ctrl+Alt+F1…F4 whatever program has the keyboard, each with its own text, history and programs | done (2026-10-06) |
| [166](../issues-done/166-exit-status-for-launchers.done) | `EXIT` with a code, `EXIT_STATUS` (58) for the last 16 tasks that ended; `grep` exits 0/1/2; `msh` makes a failed program `err` | done (2026-10-06) |
| [160](../issues-done/160-focus-for-a-started-program.done) | The task in front hands the focus to a program it starts (`SPAWN_FOREGROUND`, `loader.wit` 1.5 `commit-in-front`) and gets it back when it ends; `fm` and `console` on a full screen use it | done (2026-10-06) |
| [167](../issues-done/167-models-of-revoke-and-move.done) | TLA+ models of revoke and MOVE checked by TLC (stage II exit); a mapped memory capability no longer moves (the bug the model found) | done (2026-10-06) |
| [168](../issues-done/168-task-memory-charged-to-spawner.done) | A task's image, stack and screen are charged to its spawner's memory quota (MC-1.7) | done (2026-10-06) |
| [169](../issues-done/169-recovery-reserve.done) | A recovery reserve of the frame pool (32 MiB, set by init): applications cannot take the memory a service restart needs (MC-6.5) | done (2026-10-06) |
| [209](../issues-done/209-aarch64-smoke-interleaved-lines.done) | aarch64 smoke: a line goes out in one write, so lines of services printing at once stay whole (reported by the tools track) | done (2026-10-06) |
| [170](../issues-done/170-supervisor-without-usable-privileges.done) | The supervisor without usable privileges: init keeps the privileges it grants in escrow and holds no process control (MC-3.12, C6) | done (2026-10-06) |
| [171](../issues-done/171-limits-from-the-hardware.done) | Limits from the hardware: all RAM, every CPU, growing task, endpoint and capability tables, STAT pages (`KRN` tasks 0001–0005, 0007); `sysinfo` requested from `APP` | done (2026-10-06) |
| [171-KRN-0001](../issues-done/171-KRN-0001-ram-above-4g.done) | x86-64: task memory from all RAM, above 4 GiB too (171, step 1) | done (2026-10-06) |
| [171-KRN-0005](../issues-done/171-KRN-0005-frame-pool-ranges.done) | The frame pool takes every free range of the firmware map (171, step 5) | done (2026-10-06) |
| [171-KRN-0003](../issues-done/171-KRN-0003-every-cpu.done) | Every CPU the firmware reports (x86 up to xAPIC's 255); no tick for idle CPUs (171, step 3) | done (2026-10-06) |
| [171-KRN-0004](../issues-done/171-KRN-0004-growing-capability-tables.done) | Capability tables that grow on demand up to 4095 slots (171, step 4) | done (2026-10-06) |
| [171-KRN-0002](../issues-done/171-KRN-0002-task-and-endpoint-tables.done) | No fixed count of tasks, applications or endpoints; root quota 65 535 (171, step 2) | done (2026-10-06) |
| [171-KRN-0007](../issues-done/171-KRN-0007-stat-pages.done) | STAT from a given record on: callers page through any number of records (171) | done (2026-10-06) |
| [171-KRN-0008](../issues-done/171-KRN-0008-wake-ipis-with-many-cpus.done) | Wake IPIs with many CPUs: one per idle period, one pass over the tasks; the 16-CPU stall (171) | done (2026-10-07) |
| [171-KRN-0009](../issues-done/171-KRN-0009-sixteen-cpus-at-the-peak.done) | 16 CPUs at the peak: less work under the scheduler lock (`wake_idle` by marks, `select` over the CPU's own tasks, `STAT` in one pass); a 16-CPU clocks check (171) | done (2026-10-08) |
| [000-KRN-0010](../issues-done/000-KRN-0010-ipc-back-pressure-without-starvation.done) | IPC back-pressure without starvation: senders wait in order, no `ERR_BUSY` for a long queue (ABI 3) | done (2026-10-08) |
| [000-KRN-0011](../issues-done/000-KRN-0011-clock-calls-without-the-lock.done) | `UPTIME`, `CLOCK` and `RDTSC` without the scheduler lock, answered from the CPU's running mailbox | done (2026-10-08) |
| [211-KRN-0013](../issues-done/211-KRN-0013-fatal-messages-on-the-screen.done) | The kernel's boot lines and fatal reports on the screen too, not only on COM1 (211) | done (2026-10-08) |
| [211-KRN-0015](../issues-done/211-KRN-0015-boot-errors-on-a-mac-screen.done) | Bootloader errors and panics readable on a Mac's screen: Apple's console control set to text mode (211) | done (2026-10-08) |
| [211-KRN-0016](../issues-done/211-KRN-0016-the-screens-gop-and-boot-progress.done) | The bootloader takes the GOP of a console output (as Linux's `find_gop`), not the first listed; its progress on the text console (211) | done (2026-10-08) |
| [211-PRT-0005](../issues-done/211-PRT-0005-windows-writer-default-image.done) | The Windows writer finds its default image under `powershell -File` (Windows PowerShell 5.1) (211) | done (2026-10-08) |
| [211-PRT-0002](../issues-done/211-PRT-0002-x2apic.done) | The local APIC in x2APIC mode, as firmware leaves it; a CI group with a kernel that switches to it (211) | done (2026-10-08) |
| [211-PRT-0003](../issues-done/211-PRT-0003-tick-without-the-pit.done) | The tick from the LAPIC timer, measured on the ACPI PM timer; the PIT only without one (211) | done (2026-10-08) |
| [211-KRN-0017](../issues-done/211-KRN-0017-logs-on-the-boot-screen.done) | Service logs on the boot screen until the compositor's first frame (211) | done (2026-10-08) |
| [211-KRN-0018](../issues-done/211-KRN-0018-the-compositors-quota-fits-the-screen.done) | The compositor's memory quota fits the screen (2880 × 1800) (211) | done (2026-10-08) |
| [211-DRV-0003](../issues-done/211-DRV-0003-usb-host-on-real-hardware.done) | `usb_host` on real hardware: endpoint 0 runs again after a stall; what failed is logged (211, done for the open DRV track) | done (2026-10-08) |
| [211-KRN-0012](../issues-done/211-KRN-0012-boot-volume-identity.done) | The boot volume: the bootloader reads its own device and names the partition in `BootInfo` (ABI 4); `vfs_server` mounts only it, with the manifest the bootloader verified; the kernel stops on a bootloader of another ABI | done (2026-10-08) |
| [351-KRN-0014](../issues-done/351-KRN-0014-trial-boot-and-confirmation.done) | A trial boot: slot, trial flag and manifest digest in `BootInfo`; the kernel restarts an unconfirmed trial at its deadline; init confirms a healthy boot (`BOOT_CONFIRM`); the updater's grants split into 351-KRN-0022 | done (2026-10-08) |
| [350-KRN-0023](../issues-done/350-KRN-0023-launch-record-in-bootinfo.done) | The launch record (manifest, key, test key, images checked) in `BootInfo` (ABI 4) and in init's log line, for 350-UPD-0004 | done (2026-10-08) |
| [300-KRN-0024](../issues-done/300-KRN-0024-read-only-blockstore-client.done) | A block store client that may only read: `init` gives the shell one badged get, lent for `REQUEST_BLOCKSTORE_READ`, for 300-STO-0004 | done (2026-10-08) |
| [300-KRN-0025](../issues-done/300-KRN-0025-a-disk-for-the-block-store.done) | A disk of the block store's own: a blank or store VirtIO disk goes to `blockstore`, never to `vfs_server`; its contents survive a reboot (for 351-STO-0006) | done (2026-10-08) |
| [351-KRN-0034](../issues-done/351-KRN-0034-a-tls-client-for-programs.done) | `REQUEST_TLS`: the flag and the loader's `SLOT_TLS`, for HTTPS in programs (requested by NET; the shell's lending is APP's) | done (2026-10-09) |
| [351-KRN-0028](../issues-done/351-KRN-0028-uefi-variables-aarch64-and-dbx.done) | UEFI variables on aarch64 through AAVMF; `efivar append dbx` with a list signed by our KEK revokes a bootloader in OVMF's Secure Boot build, an unsigned one is refused | done (2026-10-09) |
| [171-KRN-0033](../issues-done/171-KRN-0033-six-gib-until-the-pool-ends.done) | At 6 GiB (x86 and aarch64) clocks start until the frame pool, not the arena, runs out, after memtest holds most of it | done (2026-10-09) |
| [171-KRN-0032](../issues-done/171-KRN-0032-kernel-structures-in-the-frame-pool.done) | Each task's kernel structures (record, pages, page tables, capability table) in the frame pool, charged to its payers; about 17 bytes of arena a task (requested by APP) | done (2026-10-09) |
| [251-KRN-0031](../issues-done/251-KRN-0031-model-disk-next-to-the-store-disk.done) | A model disk next to the store's disk, each to its own service: `virtio_blk#2` for a third VirtIO disk; `init.wit` 1.2 lists 64 services (requested by APP for 251) | done (2026-10-08) |
| [174-KRN-0038](../issues-done/174-KRN-0038-hardware-report.done) | A complete hardware report on every boot (`log:hwNNNN.txt`, `log:acpi/`); the MacBook Pro's names both GPUs and both EHCI controllers | done (2026-10-09) |
| [211-KRN-0021](../issues-done/211-KRN-0021-registers-inside-a-page.done) | Device registers that start inside a page: a BAR sharing its page with another kind of device moves to a page of its own (the MacBook Pro's AHCI and EHCI) | done (2026-10-09) |
| [211-DRV-0016](../issues-done/211-DRV-0016-hid-interfaces-not-ours-claimed-once.done) | `usb_hid` claims an interface it does not serve once, not without end (the MacBook Pro's keyboard interface 1) | done (2026-10-09) |
| [175-KRN-0046](../issues-done/175-KRN-0046-ci-fails-on-every-build-failure.done) | CI and the local gate fail on every build failure: one host-test script and one fixture script for both, each call failing in turn in `tests/gate_test.py` (audit A06) | done (2026-10-09) |
| [175-PRT-0007](../issues-done/175-PRT-0007-image-lists-programs-after-the-build.done) | The USB image lists the programs after the build: one call on a clean tree packs every program it built, checked by `tests/usb_image_test.py` (audit A01) | done (2026-10-09) |
| [175-KRN-0047](../issues-done/175-KRN-0047-fat-failed-growth-gives-clusters-back.done) | A FAT write that fails for space or on the medium gives its new clusters back and leaves the file as it was (audit A02) | done (2026-10-09) |
| [175-KRN-0048](../issues-done/175-KRN-0048-fat-case-rename-keeps-the-file.done) | A rename writes its new entry first: a refused or failed one, a change of case included, keeps the file under its old name (audit A03) | done (2026-10-09) |
| [175-KRN-0049](../issues-done/175-KRN-0049-fat-clean-only-after-a-good-flush.done) | A FAT volume reads dirty until a flush succeeds: the dirty mark flushed first, the clean mark last; power-loss and flush-failure tests on a cache model (audit A04) | done (2026-10-09) |
| [250-KRN-0056](../issues-done/250-KRN-0056-fp-simd-for-programs-on-aarch64.done) | FP/SIMD for programs on aarch64: V0–V31, FPCR and FPSR saved per task (the tools track's request, for 250 and 252) | done (2026-10-10) |
| [210-KRN-0055](../issues-done/210-KRN-0055-the-devicetree-suite-steps-through-qmp.done) | The devicetree suite steps the machine through QMP, not at the monitor's typing pace | done (2026-10-09) |
| [171-KRN-0054](../issues-done/171-KRN-0054-a-reply-to-a-slot-past-the-shrunk-task-table.done) | A reply to a client whose slot the shrunk task table dropped fails with ERR_PEER, not a kernel panic (the storage session's report) | done (2026-10-09) |
| [000-KRN-0030](../issues-done/000-KRN-0030-console-back-pressure.done) | A console program's output is not lost to a slow reader: `LOG` takes what fits while the console is read, `mind::process::log` sends the rest again (found by the local gate) | done (2026-10-08) |
| [210-KRN-0029](../issues-done/210-KRN-0029-device-tree-in-bootinfo.done) | The device tree's address in `BootInfo` (ABI 4): the bootloader passes it from the configuration table, the kernel checks its header; `devicetree` suite on `virt,acpi=off` (for 210-APL-0002) | done (2026-10-08) |
| [351-KRN-0027](../issues-done/351-KRN-0027-uefi-variables.done) | UEFI variables from the running system on x86: `FIRMWARE_VARIABLE` with the firmware privilege, lent by the shell with the user's consent; `efivar`; `BootNext` boots another entry once (requested by UPD); aarch64 and `dbx` split into 351-KRN-0028 | done (2026-10-08) |
| [000-KRN-0026](../issues-done/000-KRN-0026-busy-share-against-the-time-left.done) | The busy suite's share check measures the loop against the time other tasks leave its CPU, not the host's wall clock | done (2026-10-08) |
| [211-DRV-0009](../issues-done/211-DRV-0009-virtio-blk-every-disk.done) | `virtio_blk#1` drives the second VirtIO disk; aarch64 finds its boot volume behind another disk (211, done for the open DRV track) | done (2026-10-08) |
| [172](../issues-done/172-64-bit-handles-and-abi-version.done) | 64-bit capability handles (32-bit generation), the IPC timeout in `arg2`, 16-byte grants, ABI version 2 checked at program start | done (2026-10-07) |
| [175-STO-0012](../issues-done/175-STO-0012-no-erased-block-acknowledged.done) | A collection cut by a failure leaves no erased block acknowledged: the indexed copy leaves the index before its record is erased (audit A05) | done (2026-10-09) |
| [300-KRN-0001](../issues-done/300-KRN-0001-blockstore-at-boot.done) | The block store starts at boot over `ramdisk#1`; the shell's client in slot 25; `REQUEST_BLOCKSTORE` (requested by STO) | done (2026-10-06) |
| [300-STO-0001](../issues-done/300-STO-0001-content-identifiers.done) | Content identifiers: CIDv1 (`raw`, SHA-256) and SHA-256 in `libmind`, unsupported and non-canonical forms refused (MC-4.2, 4.13) | done (2026-10-06) |
| [301-STO-0001](../issues-done/301-STO-0001-object-format.done) | The object format: 16 KiB chunks and DAG-CBOR nodes with a shape fixed by the size, a builder and a checking reader (`mind::dag`) | done (2026-10-06) |
| [301-STO-0002](../issues-done/301-STO-0002-store-takes-nodes.done) | The block store takes nodes: `put(codec, data)`, a `dag-cbor` block stored only if it is a canonical node of `mind::dag` | done (2026-10-06) |
| [302-STO-0001](../issues-done/302-STO-0001-names-in-the-store.done) | Names in the block store: `publish` by compare-and-swap on the version, only roots whose object is complete; `resolve`; `BADGE_PUBLISH` | done (2026-10-06) |
| [300-STO-0002](../issues-done/300-STO-0002-blockstore-service.done) | The `blockstore` service: put and get by CID, append-only, every read checked; runs at boot over `ramdisk#1` | done (2026-10-07) |
| [300-STO-0003](../issues-done/300-STO-0003-blocks-tool-and-store-suite.done) | The `blocks` tool and the `store` suite on x86 and aarch64: the reference root of a 4 MiB object, a file round trip, names by compare-and-swap, a full store, a restart; `dag::Node` off the stack | done (2026-10-07) |
| [301](../issues-done/301-objects-as-merkle-dags.done) | Objects larger than a block as a Merkle-DAG (track B, second step); damage on the platform split off to 300-STO-0005 | done (2026-10-07) |
| [302](../issues-done/302-names-and-current-roots.done) | Names and their current roots by compare-and-swap, only complete roots (track B, third step); damage on the platform split off to 300-STO-0005 | done (2026-10-07) |
| [303-STO-0001](../issues-done/303-STO-0001-collection-by-reachability.done) | Collection by reachability: names retain, 60 s leases protect writes in progress, freed room reused crash-safely; `collect` in `blockstore.wit` 1.1 | done (2026-10-07) |
| [303-STO-0002](../issues-done/303-STO-0002-pins-and-quotas.done) | Pins with an owner (the badge), ended by their owner; what an owner's names and pins retain charged to it, each root once, within a quota; `blockstore.wit` 1.2 | done (2026-10-08) |
| [303-STO-0003](../issues-done/303-STO-0003-name-history.done) | A name keeps its newest 4 versions, each record linking the root before it; every kept version retains its object | done (2026-10-08) |
| [303-STO-0004](../issues-done/303-STO-0004-removing-a-name.done) | Removing a name: a version without a root, compare-and-swap like a publication; its data goes only by collection | done (2026-10-08) |
| [303](../issues-done/303-retention-and-collection.done) | Retention and garbage collection (track B, fourth step): names with history, pins, quotas per owner, removal, collection by reachability with leases | done (2026-10-08) |
| [300-STO-0005](../issues-done/300-STO-0005-corruption-on-the-platform.done) | Corruption on the platform: damage injected from the host into the store's medium in guest RAM (gdbstub); a corrupt chunk, a damaged header and a damaged name record give the host tests' outcomes on x86 and aarch64 | done (2026-10-08) |
| [304-STO-0007](../issues-done/304-STO-0007-commit-of-several-names.done) | A commit of up to 8 names, all or none: one record with one digest, applied whole when mounting; a snapshot read; `blockstore.wit` 1.3 | done (2026-10-08) |
| [304](../issues-done/304-several-names-at-once.done) | Several names at once (track B, fifth step): the boundary of atomicity, isolation and durability declared (MC-4.10) | done (2026-10-08) |
| [305-STO-0008](../issues-done/305-STO-0008-store-outside-the-boot-path.done) | The store outside the boot path: a killed store restarted by `init`, an unmountable medium answered with the reason (`stat` too) while programs start from the boot volume | done (2026-10-08) |
| [305](../issues-done/305-recovery-without-the-store.done) | Recovery without the main store (track B, sixth step): the recovery set is the boot volume; crash boundary and degradation mode (MC-6.8, B.4) | done (2026-10-08) |
| [306-STO-0009](../issues-done/306-STO-0009-checkpoint-format-and-pilot.done) | `mind::checkpoint`: a versioned manifest saved with the state in one commit, fenced by epoch, restored with contract and schema checks, an effect journal, rebinding from current grants; the pilot `tally` | done (2026-10-08) |
| [306](../issues-done/306-checkpoints-and-rebinding.done) | Checkpoints and rebinding (track B, seventh step): the contract and protocol of MC-6.10 with fencing (MC-6.12) and rebinding (MC-6.11) | done (2026-10-08) |
| [350-UPD-0001](../issues-done/350-UPD-0001-reproducible-build-check.done) | `scripts/reproducible.sh`: two builds of one commit at one path are byte-identical; the checkout path is a recorded condition | done (2026-10-08) |
| [350-UPD-0002](../issues-done/350-UPD-0002-manifest-and-signing.done) | A versioned boot manifest (files, sizes, SHA-256, requests, build inputs) signed with Ed25519 by `scripts/sign_manifest.py`; the builds sign their volumes | done (2026-10-08) |
| [350-UPD-0003](../issues-done/350-UPD-0003-verification-at-boot.done) | The bootloader checks the manifest's signature and every image it loads; a changed image, manifest, key or a missing signature stops the boot | done (2026-10-08) |
| [351-UPD-0005](../issues-done/351-UPD-0005-release-and-publish.done) | Releases: `scripts/release.py` stages, publishes (blobs, manifests, then the signed channel in one rename) and checks; a test HTTPS server; the publishing guide (EN, RU) | done (2026-10-08) |
| [351-UPD-0006](../issues-done/351-UPD-0006-slots-and-boot-records.done) | Slots A and B: the bootloader follows the newer valid boot record, counts a trial's tries on the other record before the slot runs and falls back to the confirmed slot; `scripts/boot_slots.py`; tested in QEMU on x86 and aarch64 | done (2026-10-08) |
| [351-UPD-0013](../issues-done/351-UPD-0013-release-metadata-in-libmind.done) | Release metadata in `libmind`: the channel and the boot manifest read in their one encoding and encoded again, so the updater checks the parser's answer against the signed bytes | done (2026-10-09) |
| [351-APP-0019](../issues-done/351-APP-0019-shell-release-command.done) | The shell's `release`: a channel file or a boot manifest as the parser service reads it, shown only if the answer makes the file (for 351-NET-0011) | done (2026-10-09) |
| [351-NET-0011](../issues-done/351-NET-0011-parse-release-metadata.done) | The parser service reads release channels and boot manifests (`idl/parse.wit` 1.1); a client takes the answer only if it encodes again to exactly the bytes | done (2026-10-09) |
| [351-NET-0001](../issues-done/351-NET-0001-http-downloads.done) | HTTP downloads for programs: `mind::http` (GET with `Range`, resume after a cut) and `download`, tested in QEMU (30 MiB on x86, 8 MiB on aarch64) | done (2026-10-08) |
| [351-NET-0003](../issues-done/351-NET-0003-names-in-the-network-policy.done) | Host names in `netpolicy.txt`, looked up when the grant is made at the file's `resolver`; `socket.wit` 2.3 lets the broker resolve | done (2026-10-08) |
| [351-NET-0005](../issues-done/351-NET-0005-persistent-device-key.done) | The device key kept across boots in `keystore`'s private directory of the boot disk (unencrypted until a TPM seals it); `vfs_server`'s private directories in `system/` | done (2026-10-09) |
| [351-KRN-0040](../issues-done/351-KRN-0040-keystore-private-directory.done) | `init` gives `keystore` a VFS client with its own badge | done (2026-10-09) |
| [108](../issues-done/108-editable-network-policy.done) | The network policy changed while the system runs (track D, "editable policy"): kept in the broker's private directory, a change only after the user's yes; `netpolicy.wit` 1.1 | done (2026-10-09) |
| [108-KRN-0041](../issues-done/108-KRN-0041-netpolicy-private-directory.done) | `init` gives `netpolicy` a VFS client with its own badge | done (2026-10-09) |
| [108-APP-0015](../issues-done/108-APP-0015-netpolicy-command.done) | The shell's `netpolicy [add|remove <line>]`, which asks the user first | done (2026-10-09) |
| [109-NET-0007](../issues-done/109-NET-0007-airlock-authority-map.done) | The authority map of every adapter of external input, against MC-11.11 and B.6 (`docs/network/airlock.md`) | done (2026-10-09) |
| [109-NET-0008](../issues-done/109-NET-0008-parser-service.done) | The parser service `parse`: bounded bytes in, typed messages out (`idl/parse.wit`), its endpoint and the log only | done (2026-10-09) |
| [109-KRN-0042](../issues-done/109-KRN-0042-parser-service-at-boot.done) | `init` starts `parse`; `SLOT_PARSE` 28, `REQUEST_PARSE`, the loader accepts the slot (for NET) | done (2026-10-09) |
| [109-APP-0016](../issues-done/109-APP-0016-shell-lends-the-parser.done) | The shell lends its `parse` client for `REQUEST_PARSE`; the script word `parse` (for NET) | done (2026-10-09) |
| [109-NET-0009](../issues-done/109-NET-0009-download-through-the-parser.done) | `download` parses nothing itself: response heads through `parse`, refused without it | done (2026-10-09) |
| [109](../issues-done/109-session-parsers.done) | Session parsers with minimal authority (Airlock, track D): the authority map, the parser service, `download` through it | done (2026-10-09) |
| [351-APP-0017](../issues-done/351-APP-0017-shell-lends-the-tls-client.done) | The shell lends its TLS client for `REQUEST_TLS`, with a flow grant only; the script word `tls` (for NET) | done (2026-10-09) |
| [351-NET-0002](../issues-done/351-NET-0002-https-for-programs.done) | HTTPS for programs: `download` over a `tls` session on its own grant; a server trusted by its pinned key (`tls.wit` 1.1 `connect-pinned`) or the roots | done (2026-10-09) |
| [351-KRN-0043](../issues-done/351-KRN-0043-tpm-service-at-boot.done) | `init` starts the TPM service; `PLATFORM_TPM`, `SLOT_TPM`; `keystore`'s seal client (for DRV and NET; the kernel's lookup is a request) | done (2026-10-09) |
| [351-KRN-0044](../issues-done/351-KRN-0044-update-badge.done) | The update badge `BADGE_UPDATE` in `mind::fs` (for 351-UPD-0008; `init`'s grant is a request) | done (2026-10-09) |
| [351-APP-0018](../issues-done/351-APP-0018-shell-tpm-command.done) | The shell's `tpm` command: the TPM, and a refused seal as a check (for DRV) | done (2026-10-09) |
| [300-STO-0004](../issues-done/300-STO-0004-rights-by-badge.done) | Rights to the block store by badge; a client that may only read refused `put` and `publish` on the platform | done (2026-10-09) |
| [300](../issues-done/300-checksummed-block-store.done) | A checksummed block store with content addresses (track B, first step) | done (2026-10-09) |
| [351-KRN-0022](../issues-done/351-KRN-0022-updater-grants.done) | The updater's authorities from init: TLS, the network, a restart through init, the firmware variables, and the update zone's badge (`BADGE_UPDATE`); every other `vfs_server` client init hands out badged `BADGE_READER` | done (2026-10-10) |
| [351-UPD-0008](../issues-done/351-UPD-0008-update-zone-in-vfs.done) | An update zone in `vfs_server`: the inactive slot and the boot records only, filled by the updater's stand-in on x86 and aarch64 | done (2026-10-10) |
| [251-STO-0013](../issues-done/251-STO-0013-an-index-that-grows-with-the-medium.done) | The block store's index grows with its medium: a hash table of 56-byte slots sized at mount; a 3 GiB object on the host | done (2026-10-10) |
| [251-STO-0014](../issues-done/251-STO-0014-importing-a-model-disk.done) | A model disk imported into the block store, each model one object named `models/<id>`, read back by name on x86 and aarch64 | done (2026-10-10) |
| [251-STO-0010](../issues-done/251-STO-0010-speech-models-in-the-store.done) | Speech models in the block store (main part of 251's track B work); reading into memory split as 251-STO-0015 | done (2026-10-10) |
| [211-KRN-0058](../issues-done/211-KRN-0058-a-flush-after-a-failed-one.done) | A flush after a failed one reports what it wrote: a failed run no longer leaves a stale error for the next flush (the USB image check's `sync` after a replug) | done (2026-10-10) |
| [550-DRV-0020](../issues-done/550-DRV-0020-bcm4331-read-only-probe.done) | `bcm_wifi` stage 1, a read-only probe of the MacBook Pro's BCM4331: the chip and its windows read, the 802.11 core found running, the SPROM's pins on the amplifier lines | done (2026-10-10) |

Issues 052–071 implement the [system tools plan](../docs/tools/README.md); they were numbered 032–051 on the tools branch and renumbered by [051](../issues-done/051-merge-main-into-tools.done) (each record says "Formerly tools-branch NNN."). Issues 040–043 and 045–050 were the plan's open specs on `main`; the tools records replaced them. Issues 001–011 were opened after the review of 2026-09-17 (handoff ↔ code, see [knowledge/04](../knowledge/04-handoff-vs-code-matrix.md)).
