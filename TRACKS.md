# Track registry

**Version:** 1.0 (2026-10-06)

MIND Core is developed in parallel **tracks**. Each track has its own code, its own directories and its own task numbers. **Tracks can be taken and worked on in parallel**: a session (an agent or a person) that takes a track works only in that track's directories, numbers only its own tasks and reaches `main` through the gate. How to work in a track: [AGENTS.md](AGENTS.md). The open tasks themselves: [issues/README.md](issues/README.md).

## Task numbers

| Form | What it is | Example |
|---|---|---|
| `NNN` | A **main task**: a goal the size of a roadmap step. `NNN` comes from its track's range. | `300` — a checksummed block store |
| `NNN-TRK-MMMM` | A **task** of track `TRK`: `NNN` is the main task it belongs to (`000` for none), `MMMM` is the track's own four-digit counter, never reused. | `300-STO-0002` — the `blockstore` service; `158-DRV-0001` — the drivers track's part of the video task 158 |

The track code makes every number unique, so tracks never coordinate on numbers. Several tracks may work on one main task, and each numbers its own part. Numbers given before this scheme (`158`, `205`, `u015`, …) stay as they are.

Files: `issues/NNN-short-name.md` for a main task, `issues/NNN-TRK-MMMM-short-name.md` for a task; finished ones move to `issues-done/` as `.done`.

Branches: `<tool>/<TRK>-<name>` for a track's working branch (`claude/NET-stack`), `<tool>/NNN-TRK-MMMM-<name>` for one task (`codex/300-STO-0002-blockstore`). CI runs on these names under any tool prefix.

Requests to another track: `issues/requests-<TRK>.md`. Only the owning track numbers them.

## Registry

"Open" means the track has no owner yet: it can be taken now, in parallel with the others.

| Code | Track | Roadmap | Main tasks | Directories (mainly) | Owner and branch | Status | Start with |
|---|---|---|---|---|---|---|---|
| `KRN` | kernel | A (kernel side), II | 150–199 | `kernel/` (generic part), `common/abi.rs`, `init`, `loader`, the core of `libmind` | kernel session, `claude/youthful-mendel-mf1soy` | active | [171](issues/171-limits-from-the-hardware.md) (limits from the hardware) |
| `PRT` | porting | H | 200–249 | `kernel/src/arch/`, `bootloader/` (architecture lines), `hwdocs/`, `gpio` | kernel session, `claude/youthful-mendel-mf1soy` | active | [205](issues/205-aarch64-boards.md), [207](issues/207-gpio-service.md) (runs on boards), [210](issues/210-apple-silicon-native.md) (Apple Silicon) |
| `NET` | network | D | 100–149 | `virtio_net`, `netstack`, `netpolicy`, `tls`, `keystore` | network session | active | the next step of track D in the roadmap |
| `APP` | tools | G | 250–299 (before: `u001`–`u017`) | user tools: `wm`, `fm`, `edit`, `view`, `monitor`, `pins`, voice, shell commands | tools session, `claude/wizardly-franklin-kec1a9` | active | [u017](issues/u017-pins-view.md), [requests-APP.md](issues/requests-APP.md) |
| `DRV` | drivers | A | 550–599 | ring-3 drivers: `ahci`, `nvme`, `ata`, `virtio_blk`, `virtio_input`, `usb_host`, `usb_storage`, `usb_hid`, `audio_gw`, `video_gw` | — | **open** | [158](issues/158-video-capture.md): UVC cameras over `usb_host` (isochronous transfers) |
| `STO` | state and recovery | B | 300–349 | new storage services (`blockstore`) and their `idl/` files, `docs/storage/`, `libmind` `cid`, `sha256`, `dag` and `blockstore` | storage session, `claude/relaxed-meitner-5bmhpz` | active | [300](issues/300-checksummed-block-store.md) (the service built, waiting for [requests-KRN.md](issues/requests-KRN.md)), [301](issues/301-objects-as-merkle-dags.md), [302](issues/302-names-and-current-roots.md) |
| `UPD` | update and provenance | C | 350–399 | signing and reproducible builds (`scripts/`, new tools); `bootloader/` and `loader` together with `KRN` | — | **open** | [350](issues/350-signed-boot-images.md) |
| `MRN` | Marain | E | 400–449 | `marain/` (host tools), the RFC through the maintainer | — | **open** | [400](issues/400-marain-m0-m2-host-bench.md) |
| `SAF` | safety plane | F | 450–499 | control actors and limits | — | **later** (after stage II) | — |
| `ASR` | assurance | Assurance | 500–549 | `docs/assurance/`, fuzzing and fault injection under `tests/` | — | **open** | [500](issues/500-fuzzing-abi-and-idl.md) |

New tracks get a code and a range from 600, in blocks of 50, from the maintainer (AGENTS.md, section 2).

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
