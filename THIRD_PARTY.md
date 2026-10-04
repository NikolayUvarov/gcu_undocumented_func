# Third-party material

MIND Core is licensed under MIT OR Apache-2.0 (see [README](README.md#license)). The files and dependencies below come from other projects and keep their own licences.

## Data in this repository

| File | Source | Licence | Notes |
|---|---|---|---|
| `tts/data/stress_ru.txt` | [OpenRussian dictionary](https://github.com/Badestrand/russian-dictionary) (word forms with stress); word selection by a frequency list from [hermitdave/FrequencyWords](https://github.com/hermitdave/FrequencyWords) (content CC BY-SA 4.0) | **CC BY-SA 4.0** ([text](LICENSES/CC-BY-SA-4.0.txt)) | Selected and converted by `scripts/stress_openrussian.py` and `scripts/stress_exceptions.py`. This file, and any modified version of it, is shared under CC BY-SA 4.0. It is embedded in `tts.elf`, so a distributed boot image must keep this attribution. |
| `fonts/mind-mono-16.bdf`, generated `common/font16.rs` | Subset of [Terminus Font](https://sourceforge.net/projects/terminus-font/) 4.49.1 (`ter-u16n.bdf` with the font's `alt/dv1.diff` and `alt/ij1.diff`), Copyright (C) 2020 Dimitar Toshkov Zhekov, with Reserved Font Name "Terminus Font" | **SIL Open Font License 1.1** ([text](LICENSES/OFL-1.1.txt), also [fonts/OFL.txt](fonts/OFL.txt)) | A Modified Version under the OFL (a subset of the glyphs, bitmaps unchanged), so it does not use the reserved name: it is called **MIND Mono** ([fonts/README.md](fonts/README.md)). The font stays under the OFL; code that draws with it does not. It is embedded in the programs that draw text with `mind::font16` (shell, `view`, `fm`, `edit`, the monitors and others), so a distributed boot image carries the OFL text in `LICENSES/`. |
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
