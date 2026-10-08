# Track registry

**Version:** 1.2 (2026-10-08): the Apple Silicon track `APL`, main tasks 600–649, takes 210 from `PRT`; 1.1 (2026-10-07): an open track's task can be done by an agent whose own task needs it.

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
| `KRN` | kernel | A (kernel side), II | 150–199 | `kernel/` (generic part), `common/abi.rs`, `init`, `loader`, the core of `libmind` | kernel session, `claude/youthful-mendel-mf1soy` | active | — (171 done; requests in `issues/requests-KRN.md` when other tracks file them) |
| `PRT` | porting | H | 200–249 | `kernel/src/arch/`, `bootloader/` (architecture lines), `hwdocs/`, `gpio` | kernel session, `claude/youthful-mendel-mf1soy` | active | [211](issues/211-intel-pc-from-a-sata-ssd.md) (the first real x86 PC), [205](issues/205-aarch64-boards.md), [207](issues/207-gpio-service.md) (runs on boards) |
| `NET` | network | D | 100–149 | `virtio_net`, `netstack`, `netpolicy`, `tls`, `keystore` | network session | active | the next step of track D in the roadmap |
| `APP` | tools | G | 250–299 (before: `u001`–`u017`) | user tools: `wm`, `fm`, `edit`, `view`, `monitor`, `pins`, voice, shell commands | tools session, `claude/wizardly-franklin-kec1a9` | active | [250](issues/250-voice-dictation.md): voice V3, dictation; requests in [requests-APP.md](issues/requests-APP.md) |
| `DRV` | drivers | A | 550–599 | ring-3 drivers: `ahci`, `nvme`, `ata`, `virtio_blk`, `virtio_input`, `usb_host`, `usb_storage`, `usb_hid`, `audio_gw`, `video_gw` | — | **open** | [158](issues/158-video-capture.md): UVC cameras over `usb_host` (isochronous transfers) |
| `STO` | state and recovery | B | 300–349 | new storage services (`blockstore`) and tools (`blocks`, `tally`) and their `idl/` files, `docs/storage/`, `libmind` `cid`, `sha256`, `dag`, `blockstore` and `checkpoint` | storage session, `claude/relaxed-meitner-5bmhpz` | active | Track B's steps through checkpoints are done (300–306). Waiting on other tracks: [300-STO-0004](issues/300-STO-0004-rights-by-badge.md) (a client with fewer rights, `KRN`), a durable disk for the store (`KRN`, `DRV`; [requests-KRN.md](issues/requests-KRN.md)), then [351-STO-0006](issues/351-STO-0006-releases-pinned-in-the-store.md) (releases pinned for self-update) |
| `UPD` | update and provenance | C | 350–399 | signing and reproducible builds (`scripts/`, new tools); `bootloader/` and `loader` together with `KRN` | the tracks module session (named by the maintainer, 2026-10-08), its branch recorded when it starts; worked meanwhile by the storage session at the maintainer's request (`claude/relaxed-meitner-5bmhpz`) | active | [350](issues/350-signed-boot-images.md) (the launch record in the system waits for `KRN`); [351](issues/351-self-update.md): [351-UPD-0012](issues/351-UPD-0012-secure-boot-with-our-own-keys.md) (Secure Boot) is done in QEMU and waits for a run on the maintainer's PC; [351-UPD-0006](issues-done/351-UPD-0006-slots-and-boot-records.done) (slots A and B in the bootloader) is done in QEMU; the rest waits for 351-KRN-0014 (the booted slot, the trial flag, the updater's grants), a `KRN` request for UEFI variables and `NET` |
| `MRN` | Marain | E | 400–449 | `marain/` (host tools), the RFC through the maintainer | — | **open** | [400](issues/400-marain-m0-m2-host-bench.md) |
| `SAF` | safety plane | F | 450–499 | control actors and limits | — | **later** (after stage II) | — |
| `ASR` | assurance | Assurance | 500–549 | `docs/assurance/`, fuzzing and fault injection under `tests/` | — | **open** | [500](issues/500-fuzzing-abi-and-idl.md) |
| `APL` | Apple Silicon | H | 600–649; 210 (numbered before) | `docs/apple-silicon.md` and `apple-silicon_RU.md`; the macOS side of `scripts/build_aarch64.sh` and `03_run_qemu_aarch64.sh`, and Apple's devices in `kernel/src/arch/aarch64/` (device tree, AIC, spin table, DART, watchdog, UART), together with `PRT`; `BootInfo`, DMA regions and the QEMU test harness through `KRN`; `usb_host` for the Type-C ports as `DRV` tasks | — | **open**; its tasks need a person with a Mac ([issues-human](issues-human/README.md#4-a-mac-with-apple-silicon-to-test-on)) | [600](issues/600-apple-silicon-mac-vm-host.md) (a virtual machine on a Mac; guide: [docs/apple-silicon.md](docs/apple-silicon.md)), [210](issues/210-apple-silicon-native.md) (natively) |

New tracks get a code and a range from 600, in blocks of 50, from the maintainer (AGENTS.md, section 2); the next free block is 650–699.

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
