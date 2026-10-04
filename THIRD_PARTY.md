# Third-party material

MIND Core is licensed under MIT OR Apache-2.0 (see [README](README.md#license)). The files and dependencies below come from other projects and keep their own licences.

## Data in this repository

| File | Source | Licence | Notes |
|---|---|---|---|
| `tts/data/stress_ru.txt` | [OpenRussian dictionary](https://github.com/Badestrand/russian-dictionary) (word forms with stress); word selection by a frequency list from [hermitdave/FrequencyWords](https://github.com/hermitdave/FrequencyWords) (content CC BY-SA 4.0) | **CC BY-SA 4.0** ([text](LICENSES/CC-BY-SA-4.0.txt)) | Selected and converted by `scripts/stress_openrussian.py` and `scripts/stress_exceptions.py`. This file, and any modified version of it, is shared under CC BY-SA 4.0. It is embedded in `tts.elf`, so a distributed boot image must keep this attribution. |
| `tts/data/lexicon_en.txt` | [CMU Pronouncing Dictionary](https://github.com/cmusphinx/cmudict), Copyright (C) 1993-2015 Carnegie Mellon University; word selection by a frequency list from hermitdave/FrequencyWords | **BSD-style** ([text](LICENSES/CMUdict-BSD.txt)) | Converted to the tts phoneme alphabet by `scripts/lexicon_en.py`. Redistributions in source or binary form (it is embedded in `tts.elf`) must reproduce the CMU copyright notice and disclaimer. |

Everything else was written for this project, including the 8×8 bitmap font in `common/font.rs`, which was drawn for MIND Core.

## Rust dependencies (fetched by Cargo, not stored here)

| Crate | Used by | Licence |
|---|---|---|
| `uefi` 0.27, `uefi-raw`, `uefi-macros` | `bootloader` | MPL-2.0 |
| `linked_list_allocator` 0.10, `spinning_top`, `lock_api`, `scopeguard` | `kernel` | MIT OR Apache-2.0 |
| `ucs2` 0.3 | `bootloader` (through `uefi`) | MPL-2.0 |
| `log`, `bitflags`, `uguid`, `bit_field`, `ptr_meta` (MIT only) | `bootloader` (through `uefi`) | MIT OR Apache-2.0 |
| `syn`, `quote`, `proc-macro2`, `unicode-ident` (also Unicode-3.0) | build time only (procedural macros of `uefi`) | MIT OR Apache-2.0 |

MPL-2.0 is a file-level copyleft: it applies to those crates' own files, not to MIND Core. The exact versions are pinned in each `Cargo.lock`.

## Tools used at build or test time (not distributed)

- OVMF (EDK II UEFI firmware, BSD-2-Clause-Patent) to boot QEMU; installed by the system package manager or placed next to the launch scripts.
- QEMU, Rust toolchain, Python.
- Optional: a Vosk speech model for the `tts` test (`--asr-model`).
