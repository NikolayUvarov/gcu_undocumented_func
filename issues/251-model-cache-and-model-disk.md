# 251 — Speech models: a cache on the host, a model disk for the system, the block store later

**Type:** main task, tools · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Roadmap:** track G (voice V3 and synthesis), track B (the block store, later) · **Constitution:** MC-4.2, MC-9.2, MC-11.11, MC-12.1

## Problem

The speech models chosen for dictation ([250](250-voice-dictation.md)) and for synthesis ([252](252-neural-speech-synthesis.md)) weigh from 25 MB to 3 GB. They cannot go in git, and nobody should have to fetch them from the network on every machine or every CI run. The maintainer asked for three things on 2026-10-08:

- a local store of models that can be copied and carried;
- a separate model disk for the system;
- the models in MIND's own block store, later.

The same day, the maintainer also decided that any free licence is allowed. Terms that limit use are recorded with each model.

## What is done (251-APP-0009, 2026-10-08)

- **`models/manifest.toml`**: every model with
  - a stable id, its role (recognition or synthesis), languages and variant (`compact`: size first; `quality`: accuracy and pronunciation first);
  - its engine, the licence of its weights, any terms of its training data, and what was measured;
  - its source pinned to a revision, and every file with its size and SHA-256.
- **`scripts/models.py`**, Python 3.11 standard library only:
  - `list`: what the manifest names, and whether it is cached;
  - `fetch`: downloads missing files, or copies them with `--from` from another cache, a pack or a mounted model disk;
  - `verify`: hashes every cached file again;
  - `pack`: one tar file to carry;
  - `disk`: a FAT32 image for MIND Core;
  - `pin`: prints a manifest entry for files of a Hugging Face repository.

  A selection is `--variant`, `--role`, `--lang` or model ids. Every file is checked against its SHA-256 before it is used, whatever it came from.
- **The cache:** `$MIND_MODELS`, else `~/.cache/mind-models`. It holds `<id>/<path>` and a `MANIFEST.json` of what is present. Copying the directory, or `pack` and `fetch --from`, carries it to another machine.
- **`scripts/fat32.py`:** the disk image.
  - An MBR with one FAT32 partition (type 0x0C) at 1 MiB, holding contiguous files with long names, `MANIFEST.json` at the root, and the volume label `MIND MODELS`.
  - Timestamps and the volume id are fixed, so the same models give the same image. The file is sparse.
  - QEMU's `vvfat`, which `make_usb_image.py` uses, stops at about 516 MB; this writer has no such limit.
  - Checked: 305 files, nested directories and Cyrillic names with spaces came back byte for byte through `mtools`, and `fsck.fat` found no error. A disk of the Russian Vosk model mounted the same way.

## Plan

1. **The models of 252** (done in 252-APP-0011): the eight voices the maintainer chose, with the models they need. The manifest now also takes `variant` lists, `voices` and `needs`, and tar archives.
2. **`models:` in the system** (`vfs_server`; done on x86 in [251-APP-0010](../issues-done/251-APP-0010-models-volume.done)).
   - After the boot volume, `vfs_server` mounts the first other FAT volume labelled `MIND MODELS` as `models:`, read-only whatever the device allows.
   - It logs `[VFS] MOUNTED MODELS: <n> MB` and reads `MANIFEST.json`.
   - On x86 nothing else is needed: the boot disk is on `ata`, and a model disk on `virtio_blk` already reaches `vfs_server`.
   - `03_run_qemu.sh` attaches `$MIND_MODELS_DISK` as a read-only virtio disk, and a QEMU check reads a model file and compares its SHA-256 with the manifest.
3. **A second virtio disk** (aarch64, where the boot disk is already virtio): `virtio_blk` serves each device as its own instance (`virtio_blk#1`), as `virtio_net` does.
   - This is the drivers track's task, open and without an owner, so the tools track does it for this one (AGENTS.md, section 5).
   - `init` then has to give `vfs_server` the second instance's client, which is a request to the kernel track ([requests-KRN.md](requests-KRN.md)).
4. **Models as memory objects.** A recognizer or synthesizer maps a model file read-only and shares it ([150](../issues-done/150-user-memory-beyond-the-arena.done)). Its hash is checked once against `MANIFEST.json` before use.
   - Reading must get faster first. On 2026-10-08 `sha256` read `models:` at about 0.2 MB/s: 16 MiB took 82 s under QEMU without KVM (252-APP-0011).
   - At that rate Vosk TTS 0.9's 937 MB would take over an hour. Larger block reads in `vfs_server` and fewer calls per file are the first things to measure.
5. **The block store** (track B, storage track). A model disk is imported into the store as named, pinned objects, one name per model id, and models are read from the store by name and CID. Requested from the storage track ([requests-STO.md](requests-STO.md)).

Tasks are numbered `251-APP-MMMM` from 0010.

## Acceptance criteria

- `models.py fetch`, `verify`, `pack`, `fetch --from` and `disk` work for every model of the manifest on a fresh host, and every file matches its SHA-256 (MC-4.2: content verified before use).
- With a model disk attached, MIND Core on x86 and on aarch64 (QEMU) shows `models:` read-only, and a model file read there matches its SHA-256.
- No model file is in git. Each model's licence and terms reach the system with it, in `MANIFEST.json`.

## Related

[250](250-voice-dictation.md), [252](252-neural-speech-synthesis.md), [scripts/voice_v3](../scripts/voice_v3/README.md), [150](../issues-done/150-user-memory-beyond-the-arena.done), [300](300-checksummed-block-store.md).
