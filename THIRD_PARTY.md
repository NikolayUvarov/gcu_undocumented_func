# Third-party material

MIND Core is licensed under MIT OR Apache-2.0 (see [README](README.md#license)). The files and dependencies below come from other projects and keep their own licences.

## Data in this repository

| File | Source | Licence | Notes |
|---|---|---|---|
| `phonetics/data/stress_ru.txt` | [OpenRussian dictionary](https://github.com/Badestrand/russian-dictionary) (word forms with stress); word selection by a frequency list from [hermitdave/FrequencyWords](https://github.com/hermitdave/FrequencyWords) (content CC BY-SA 4.0) | **CC BY-SA 4.0** ([text](LICENSES/CC-BY-SA-4.0.txt)) | Selected and converted by `scripts/stress_openrussian.py` and `scripts/stress_exceptions.py`. This file, and any modified version of it, is shared under CC BY-SA 4.0. It is embedded in `tts.elf`, `hear.elf` and `voice.elf`, so a distributed boot image must keep this attribution. |
| `fonts/mind-mono-16.bdf`, generated `common/font16.rs` | Subset of [Terminus Font](https://sourceforge.net/projects/terminus-font/) 4.49.1 (`ter-u16n.bdf` with the font's `alt/dv1.diff` and `alt/ij1.diff`), Copyright (C) 2020 Dimitar Toshkov Zhekov, with Reserved Font Name "Terminus Font" | **SIL Open Font License 1.1** ([text](LICENSES/OFL-1.1.txt), also [fonts/OFL.txt](fonts/OFL.txt)) | A Modified Version under the OFL (a subset of the glyphs, bitmaps unchanged), so it does not use the reserved name: it is called **MIND Mono** ([fonts/README.md](fonts/README.md)). The font stays under the OFL; code that draws with it does not. It is embedded in the programs that draw text with `mind::font16` (shell, `view`, `fm`, `edit`, the monitors and others), so a distributed boot image carries the OFL text in `LICENSES/`. |
| `phonetics/data/lexicon_en.txt` | [CMU Pronouncing Dictionary](https://github.com/cmusphinx/cmudict), Copyright (C) 1993-2015 Carnegie Mellon University; word selection by a frequency list from hermitdave/FrequencyWords | **BSD-style** ([text](LICENSES/CMUdict-BSD.txt)) | Converted to the tts phoneme alphabet by `scripts/lexicon_en.py`. Redistributions in source or binary form (it is embedded in `tts.elf`, `hear.elf` and `voice.elf`) must reproduce the CMU copyright notice and disclaimer. |
| `voice/model.bin` | Trained by `scripts/voice_train.rs` on speech from this project's synthesizer only (no recordings, no outside model); the synthesizer's pronunciations come from the two dictionaries above. It holds no text of them | **CC BY-SA 4.0**, to be safe ([text](LICENSES/CC-BY-SA-4.0.txt)) | The model may count as an adaptation of `stress_ru.txt` (its stresses shaped the training speech), so it is shared under that file's licence with its attribution, and the CMU notice travels with it as for `lexicon_en.txt`. Retrain it with the script to change it. |
| `hwdocs/socs/bcm2711.pins` | Transcribed from "BCM2711 ARM Peripherals" (Raspberry Pi Ltd, RP-008248-DS-1), section 5.3, table 94: the pins' alternate functions and pulls at reset | facts about the hardware; no text or figures copied | not in the system image; put on a disk with `make_usb_image.py --hwdocs` |
| `hwdocs/boards/rpi4b.board` | Transcribed from "Raspberry Pi 4 Model B Reduced Schematics" (Raspberry Pi Ltd, RP-008345-DS-1), connector J8: which GPIO is at which header position | as above | as above |
| `hwdocs/socs/pl061.pins` | Arm's "PrimeCell GPIO (PL061) Technical Reference Manual" (DDI 0190B): eight pins, no multiplexing | as above | as above |

Code adapted from another project:

| File | Source | Licence |
|---|---|---|
| `tls/src/provider.rs` | The rustls crypto provider of [rustls-rustcrypto](https://github.com/RustCrypto/rustls-rustcrypto) 0.0.2-alpha (RustCrypto Developers): cipher, hash, HMAC, key exchange and signature verification glue, rewritten for TLS 1.3 only, RDRAND randomness and no private keys | MIT OR Apache-2.0 |

Everything else was written for this project, including the 8×8 bitmap font in `common/font.rs`, which was drawn for MIND Core.

## Rust dependencies (fetched by Cargo, not stored here)

| Crate | Used by | Licence |
|---|---|---|
| `uefi` 0.27, `uefi-raw`, `uefi-macros` | `bootloader` | MPL-2.0 |
| `linked_list_allocator` 0.10, `spinning_top`, `lock_api`, `scopeguard` | `kernel` | MIT OR Apache-2.0 |
| `smoltcp` 0.14 (TCP/IP), `managed` 0.8 | `netstack` | 0BSD |
| `heapless`, `hash32`, `stable_deref_trait`, `cfg-if`, `bitflags` 1.3 | `netstack` (through `smoltcp`) | MIT OR Apache-2.0 |
| `byteorder` | `netstack` (through `smoltcp`) | Unlicense OR MIT |
| `rustls` 0.23 (TLS 1.3 client) | `tls` | Apache-2.0 OR ISC OR MIT |
| `rustls-webpki` 0.103, `untrusted` 0.9 | `tls` (certificate verification, through `rustls`) | **ISC** ([text](LICENSES/ISC-webpki.txt)) |
| `rustls-pki-types` | `tls` | MIT OR Apache-2.0 |
| RustCrypto: `aead`, `aes`, `aes-gcm`, `chacha20`, `chacha20poly1305`, `poly1305`, `polyval`, `ghash`, `ctr`, `cipher`, `universal-hash`, `inout`, `sha2`, `hmac`, `hkdf`, `digest`, `block-buffer`, `crypto-common`, `p256`, `p384`, `ecdsa`, `elliptic-curve`, `primeorder`, `sec1`, `rfc6979`, `crypto-bigint`, `ff`, `group`, `base16ct`, `der`, `spki`, `pkcs1`, `pkcs8`, `const-oid`, `signature`, `ed25519`, `rsa`, `zeroize`, `opaque-debug`, `cpufeatures` | `tls`, `keystore` | MIT OR Apache-2.0 |
| `curve25519-dalek` 4, `x25519-dalek` 2, `ed25519-dalek` 2, `subtle` 2 | `tls`, `keystore` | **BSD-3-Clause** ([text](LICENSES/BSD-3-Clause-dalek.txt)) |
| `num-bigint-dig`, `num-integer`, `num-iter`, `num-traits`, `rand`, `rand_chacha`, `rand_core`, `ppv-lite86`, `lazy_static`, `once_cell`, `smallvec`, `typenum`, `zerocopy` (also BSD-2-Clause) | `tls` (through `rsa` and `rustls`) | MIT OR Apache-2.0 |
| `generic-array`, `spin`, `libm` | `tls` (through the RustCrypto crates and `rsa`) | MIT |
| `ucs2` 0.3 | `bootloader` (through `uefi`) | MPL-2.0 |
| `log`, `bitflags`, `uguid`, `bit_field`, `ptr_meta` (MIT only) | `bootloader` (through `uefi`) | MIT OR Apache-2.0 |
| `syn`, `quote`, `proc-macro2`, `unicode-ident` (also Unicode-3.0) | build time only (procedural macros of `uefi`) | MIT OR Apache-2.0 |

`rsa` only verifies signatures with public keys in `tls`; the timing side channel of its private-key operations (RUSTSEC-2023-0071) does not apply. The BSD-3-Clause and ISC crates are compiled into `tls.elf` and `keystore.elf`, so a distributed boot image carries their notices (`LICENSES/`).

MPL-2.0 is a file-level copyleft: it applies to those crates' own files, not to MIND Core. The exact versions are pinned in each `Cargo.lock`.

## Tools used at build or test time (not distributed)

- OVMF (EDK II UEFI firmware, BSD-2-Clause-Patent) to boot QEMU; installed by the system package manager or placed next to the launch scripts.
- QEMU, Rust toolchain, Python.
- The TLA+ tools (`tla2tools.jar` 1.8.0, MIT licence, [tlaplus/tlaplus](https://github.com/tlaplus/tlaplus)) to check the models of `docs/assurance`; `scripts/model_check.sh` fetches them and checks their SHA-256; Java.
- Optional: a Vosk speech model for the `tts` test (`--asr-model`).
