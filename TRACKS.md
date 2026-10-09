# Track registry

**Version:** 1.5 (2026-10-09): `ASR` and `DRV` taken by the assessing session (`claude/ASR-DRV`) at the maintainer's request; 1.4 (2026-10-09): the Apple Silicon track `APL` parked (status "later") by the maintainer; 1.3 (2026-10-08): the track `DEV`, on-target development, main tasks 650–699, proposed; 1.2 (2026-10-08): the Apple Silicon track `APL`, main tasks 600–649, takes 210 from `PRT`; 1.1 (2026-10-07): an open track's task can be done by an agent whose own task needs it.

MIND Core is developed in parallel **tracks**. Each track has its own code, its own directories and its own task numbers. **Tracks can be taken and worked on in parallel**: a session (an agent or a person) that takes a track works only in that track's directories, numbers only its own tasks and reaches `main` through the gate. How to work in a track: [AGENTS.md](AGENTS.md). The open tasks themselves: [issues/README.md](issues/README.md).

## Task numbers

| Form | What it is | Example |
|---|---|---|
| `NNN` | A **main task**: a goal the size of a roadmap step. `NNN` comes from its track's range. | `300` — a checksummed block store |
| `NNN-TRK-MMMM` | A **task** of track `TRK`: `NNN` is the main task it belongs to (`000` for none), `MMMM` is the track's own four-digit counter, never reused. | `300-STO-0002` — the `blockstore` service; `158-DRV-0001` — the drivers track's part of the video task 158 |

The track code makes every number unique, so tracks never coordinate on numbers. Several tracks may work on one main task, and each numbers its own part. Numbers given before this scheme (`158`, `205`, `u015`, …) stay as they are.

Files: `issues/NNN-short-name.md` for a main task, `issues/NNN-TRK-MMMM-short-name.md` for a task; finished ones move to `issues-done/` as `.done`.

Branches: `<tool>/<TRK>-<name>` for a track's working branch (`claude/NET-stack`), `<tool>/NNN-TRK-MMMM-<name>` for one task (`codex/300-STO-0002-blockstore`). CI runs on these names under any tool prefix.

Requests to another track: `issues/requests-<TRK>.md`. Only the owning track numbers them. A track without an owner has no one to number them: an agent whose own task needs a change there makes it as that track's task, numbered with the track's code ([AGENTS.md](AGENTS.md), section 5).

## Registry

"Open" means the track has no owner yet: it can be taken now, in parallel with the others. Until then, an agent whose own task needs a change in an open track's directories makes that change as one of the track's tasks, without taking the track ([AGENTS.md](AGENTS.md), section 5).

| Code | Track | Roadmap | Main tasks | Directories (mainly) | Owner and branch | Status | Start with |
|---|---|---|---|---|---|---|---|
| `KRN` | kernel | A (kernel side), II | 150–199 | `kernel/` (generic part), `common/abi.rs`, `init`, `loader`, `vfs_server` (since 2026-10-09, [175](issues/175-audit-2026-10-09.md)), the core of `libmind`, CI | kernel session, `claude/youthful-mendel-mf1soy` | active | — (171 done; requests in `issues/requests-KRN.md` when other tracks file them) |
| `PRT` | porting | H | 200–249 | `kernel/src/arch/`, `bootloader/` (architecture lines), `hwdocs/`, `gpio` | kernel session, `claude/youthful-mendel-mf1soy` | active | [211](issues/211-intel-pc-from-a-sata-ssd.md) (the first real x86 PC), [205](issues/205-aarch64-boards.md), [207](issues/207-gpio-service.md) (runs on boards) |
| `NET` | network | D | 100–149 | `virtio_net`, `netstack`, `netpolicy`, `parse`, `tls`, `keystore`, `download`, `netcheck`, `libmind` `http`, `parse` and `network` | network session; worked meanwhile by the storage session at the maintainer's request (`claude/relaxed-meitner-5bmhpz`) | active | for [351](issues/351-self-update.md): [351-NET-0001](issues-done/351-NET-0001-http-downloads.done) (HTTP downloads with resume) is done; [351-NET-0003](issues-done/351-NET-0003-names-in-the-network-policy.done) (names in the policy) is done; [351-NET-0002](issues-done/351-NET-0002-https-for-programs.done) (HTTPS for programs, a pinned key for the update server) is done; [351-NET-0005](issues-done/351-NET-0005-persistent-device-key.done) (the device key kept on the disk, the interim) is done, [351-NET-0006](issues/351-NET-0006-device-key-sealed-by-a-tpm.md) (sealed by a TPM) and the `tpm` service of the open drivers track (351-DRV-0015) are built and wait for the kernel to hand out the TPM's registers (requests-KRN.md); 351-NET-0004 (SSH) is phase 3; track D's editable policy ([108](issues-done/108-editable-network-policy.done)) and session parsers ([109](issues-done/109-session-parsers.done)) are done, [109-NET-0010](issues/109-NET-0010-a-parser-per-session.md) (a parser per session) waits for a kernel change |
| `APP` | tools | G | 250–299 (before: `u001`–`u017`) | user tools: `wm`, `fm`, `edit`, `view`, `monitor`, `pins`, voice, shell commands | tools session, `claude/wizardly-franklin-kec1a9` | active | [250](issues/250-voice-dictation.md): voice V3, dictation; [251](issues/251-model-cache-and-model-disk.md): speech models in a cache and on a model disk; [252](issues/252-neural-speech-synthesis.md): neural speech synthesis; requests in [requests-APP.md](issues/requests-APP.md) |
| `DRV` | drivers | A | 550–599 | ring-3 drivers: `ahci`, `nvme`, `ata`, `virtio_blk`, `virtio_input`, `usb_host`, `usb_storage`, `usb_hid`, `audio_gw`, `video_gw` | the assessing session, `claude/ASR-DRV` (2026-10-09). DRV tasks already worked in other sessions stay with them until done: 211-DRV-0004, 0008, 0016–0018 and 551-DRV-0010 (kernel session), 351-DRV-0015 (storage session) | active | [211-DRV-0002](issues/211-DRV-0002-ahci-every-port.md): every AHCI port, the boot disk by its identity; [550-DRV-0005](issues/550-DRV-0005-usb-ethernet.md): USB Ethernet; [551](issues/551-sound-on-pcs.md): Intel HD Audio for speakers and microphone (551-DRV-0010); [158](issues/158-video-capture.md): UVC cameras over `usb_host` (isochronous transfers), the MacBook Pro's camera |
| `STO` | state and recovery | B | 300–349 | new storage services (`blockstore`) and tools (`blocks`, `tally`) and their `idl/` files, `docs/storage/`, `libmind` `cid`, `sha256`, `dag`, `blockstore`, `checkpoint`, `json` and `models` | storage session, `claude/relaxed-meitner-5bmhpz` | active | Track B's steps through checkpoints are done (300–306, [300-STO-0004](issues-done/300-STO-0004-rights-by-badge.done) included), and the store has a disk of its own (300-KRN-0025). Next: [251-STO-0010](issues/251-STO-0010-speech-models-in-the-store.md) (speech models in the store); [351-STO-0006](issues/351-STO-0006-releases-pinned-in-the-store.md) (releases pinned) waits for the updater (351-UPD-0007); [300-STO-0011](issues/300-STO-0011-a-client-that-may-only-store.md) for a program that only stores |
| `UPD` | update and provenance | C | 350–399 | signing and reproducible builds (`scripts/`, new tools), `libmind` `release`; `bootloader/` and `loader` together with `KRN` | the tracks module session (named by the maintainer, 2026-10-08), its branch recorded when it starts; worked meanwhile by the storage session at the maintainer's request (`claude/relaxed-meitner-5bmhpz`) | active | [350](issues/350-signed-boot-images.md) (the launch record in the system waits for `KRN`); [351](issues/351-self-update.md): [351-UPD-0012](issues/351-UPD-0012-secure-boot-with-our-own-keys.md) (Secure Boot) is done in QEMU and waits for a run on the maintainer's PC; [351-UPD-0006](issues-done/351-UPD-0006-slots-and-boot-records.done) (slots A and B in the bootloader) is done in QEMU; the rest waits for 351-KRN-0014 (the booted slot, the trial flag, the updater's grants), a `KRN` request for UEFI variables and `NET` |
| `MRN` | Marain | E | 400–449 | `marain/` (host tools), the RFC through the maintainer | — | **open** | [400](issues/400-marain-m0-m2-host-bench.md) |
| `SAF` | safety plane | F | 450–499 | control actors and limits | — | **later** (after stage II) | — |
| `ASR` | assurance | Assurance | 500–549 | `docs/assurance/`, fuzzing and fault injection under `tests/` | the assessing session, `claude/ASR-DRV` (2026-10-09) | active | [500](issues/500-fuzzing-abi-and-idl.md): 500-ASR-0001, fuzzing the IDL decoders, first; [351-ASR-0006](issues/351-ASR-0006-update-threat-model.md): the update threat model |
| `APL` | Apple Silicon | H | 600–649; 210 (numbered before) | `docs/apple-silicon.md` and `apple-silicon_RU.md`; the macOS side of `scripts/build_aarch64.sh` and `03_run_qemu_aarch64.sh`, and Apple's devices in `kernel/src/arch/aarch64/` (device tree, AIC, spin table, DART, watchdog, UART), together with `PRT`; `BootInfo`, DMA regions and the QEMU test harness through `KRN`; `usb_host` for the Type-C ports as `DRV` tasks | — | **later**: parked by the maintainer on 2026-10-09. Every Apple Silicon Mac boots only through iBoot. A bare-metal start needs Asahi's m1n1 and U-Boot set up once on the internal disk, and they exist for M1 to M3 only. No Mac whose boot chain may be changed is at hand: the one at hand is an M5. The goal stays bare metal; a virtual machine (600) is not the aim. Its tasks need a person with such a Mac ([issues-human](issues-human/README.md#4-a-mac-with-apple-silicon-to-test-on)) | [600](issues/600-apple-silicon-mac-vm-host.md) (a virtual machine on a Mac; guide: [docs/apple-silicon.md](docs/apple-silicon.md)), [210](issues/210-apple-silicon-native.md) (natively) |
| `DEV` | on-target development | — (proposed 2026-10-08; the maintainer confirms the code and range) | 650–699 | new tools for building on the target: a git client, a POSIX layer for ported programs, the Rust toolchain port | — | **proposed** | [650](issues/650-building-on-the-target.md) |

New tracks get a code and a range from 600, in blocks of 50, from the maintainer (AGENTS.md, section 2); the next free block is 700–749.

## Taking a track

1. Tell the maintainer which open track you take. The maintainer writes the owner and the branch into this table.
2. Paste the brief from [AGENTS.md](AGENTS.md), section 7, with the track's code, range, directories and branch.
3. Start from the track's main task. Split it into `NNN-TRK-0001`, `NNN-TRK-0002`, … as you go.

## What runs in parallel and what does not

- **Tracks in parallel.** Every track works in its own directories, so tracks merge without conflicts.
- **One session per track** in shared files. The kernel files (`kernel/src/scheduler.rs`, `common/abi.rs`) are changed only by `KRN`. A second kernel session would conflict on every task.
- **Dependencies go through requests, not through edits.** Examples:
  - a system call for `STO` is a `KRN` task;
  - a change in the bootloader for `UPD` is done together with `KRN`/`PRT`.
- **The storage session's standing permission (the maintainer, 2026-10-09).**
  - The storage session (`claude/relaxed-meitner-5bmhpz`) works every track that no other session holds, switching between them as its work needs. That excludes the kernel and tools tracks, which have sessions of their own.
  - When its own task needs a small change in `KRN`'s or `APP`'s area (grant lines in `init`, a constant in `common/abi.rs` or `libmind`, a shell command), it makes the change as a numbered task of that track. It numbers from `KRN-0040` and `APP-0015`, so as not to meet those sessions' own numbers, and says so in the task.
  - Everything else for those tracks still goes through their requests.
