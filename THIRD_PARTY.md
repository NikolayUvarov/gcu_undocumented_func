# Third-party material

MIND Core is licensed under MIT OR Apache-2.0 (see [README](README.md#license)). The files and dependencies below come from other projects and keep their own licences.

## Data in this repository

| File | Source | Licence | Notes |
|---|---|---|---|
| `phonetics/data/stress_ru.txt` | [OpenRussian dictionary](https://github.com/Badestrand/russian-dictionary) (word forms with stress); word selection by a frequency list from [hermitdave/FrequencyWords](https://github.com/hermitdave/FrequencyWords) (content CC BY-SA 4.0) | **CC BY-SA 4.0** ([text](LICENSES/CC-BY-SA-4.0.txt)) | Selected and converted by `scripts/stress_openrussian.py` and `scripts/stress_exceptions.py`. This file, and any modified version of it, is shared under CC BY-SA 4.0. It is embedded in `tts.elf`, `hear.elf` and `voice.elf`, so a distributed boot image must keep this attribution. |
| `fonts/mind-mono-16.bdf`, generated `common/font16.rs` | Subset of [Terminus Font](https://sourceforge.net/projects/terminus-font/) 4.49.1 (`ter-u16n.bdf` with the font's `alt/dv1.diff` and `alt/ij1.diff`), with one glyph of the project's own (⚡ U+26A1), Copyright (C) 2020 Dimitar Toshkov Zhekov, with Reserved Font Name "Terminus Font" | **SIL Open Font License 1.1** ([text](LICENSES/OFL-1.1.txt), also [fonts/OFL.txt](fonts/OFL.txt)) | A Modified Version under the OFL (a subset of the glyphs, bitmaps unchanged), so it does not use the reserved name: it is called **MIND Mono** ([fonts/README.md](fonts/README.md)). The font stays under the OFL; code that draws with it does not. It is embedded in the programs that draw text with `mind::font16` (shell, `view`, `fm`, `edit`, the monitors and others), so a distributed boot image carries the OFL text in `LICENSES/`. |
| `phonetics/data/lexicon_en.txt` | [CMU Pronouncing Dictionary](https://github.com/cmusphinx/cmudict), Copyright (C) 1993-2015 Carnegie Mellon University; word selection by a frequency list from hermitdave/FrequencyWords | **BSD-style** ([text](LICENSES/CMUdict-BSD.txt)) | Converted to the tts phoneme alphabet by `scripts/lexicon_en.py`. Redistributions in source or binary form (it is embedded in `tts.elf`, `hear.elf` and `voice.elf`) must reproduce the CMU copyright notice and disclaimer. |
| `voice/model.bin` | Trained by `scripts/voice_train.rs` on speech from this project's synthesizer only (no recordings, no outside model); the synthesizer's pronunciations come from the two dictionaries above. It holds no text of them | **CC BY-SA 4.0**, to be safe ([text](LICENSES/CC-BY-SA-4.0.txt)) | The model may count as an adaptation of `stress_ru.txt` (its stresses shaped the training speech), so it is shared under that file's licence with its attribution, and the CMU notice travels with it as for `lexicon_en.txt`. Retrain it with the script to change it. |
| `hwdocs/socs/bcm2711.pins` | Transcribed from "BCM2711 ARM Peripherals" (Raspberry Pi Ltd, RP-008248-DS-1), section 5.3, table 94: the pins' alternate functions and pulls at reset | facts about the hardware; no text or figures copied | not in the system image; put on a disk with `make_usb_image.py --hwdocs` |
| `hwdocs/boards/rpi4b.board` | Transcribed from "Raspberry Pi 4 Model B Reduced Schematics" (Raspberry Pi Ltd, RP-008345-DS-1), connector J8: which GPIO is at which header position | as above | as above |
| `hwdocs/socs/pl061.pins` | Arm's "PrimeCell GPIO (PL061) Technical Reference Manual" (DDI 0190B): eight pins, no multiplexing | as above | as above |

Code adapted from another project:

| File | Source | Licence |
|---|---|---|
| `rtl8168/src/controller.rs` | RTL8168G reset, register setup and standard PHY negotiation adapted from FreeBSD commit `16a79708d089423ec0c3d32f25e7ca97ae58e97b`, `sys/dev/re/if_re.c`, `sys/dev/rl/if_rlreg.h`, `sys/dev/mii/rgephy.c` and `rgephyreg.h`, Copyright (c) 1997, 1998-2003 Bill Paul; [scope and source hashes](hwdocs/rtl8168.md#bounded-rtl8168g-service) | **BSD-4-Clause** ([notices](LICENSES/BSD-4-Clause-realtek.txt)); This product includes software developed by Bill Paul. |
| `tls/src/provider.rs` | The rustls crypto provider of [rustls-rustcrypto](https://github.com/RustCrypto/rustls-rustcrypto) 0.0.2-alpha (RustCrypto Developers): cipher, hash, HMAC, key exchange and signature verification glue, rewritten for TLS 1.3 only, RDRAND randomness and no private keys | MIT OR Apache-2.0 |
| `rtw89_wifi/src/{firmware,registers,power,efuse,fwcmd,mac,pcie,download,runtime_mac,runtime_mac_regs,rx,rxdesc,c2h,post_mac,post_mac_regs,report_tail,phycap,h2c,btc,btc_init}.rs` | RTL8852BE firmware format, register definitions, initial MAC power/XTAL/EFUSE sequences, MAC download-memory/PCIe preparation, WCPU firmware download and command transport adapted from Linux v6.19 commit `05f7e89ab9731565d8a62e3b5d1ec206485eeb0b`, `drivers/net/wireless/realtek/rtw89/{fw.h,fw.c,core.h,core.c,reg.h,mac.h,mac.c,efuse.c,rtw8852b.c,rtw8852b_common.c,rtw8852b_common.h,pci.c,pci.h,txrx.h,rtw8852be.c,coex.c}`, Copyright(c) 2019-2020, 2019-2022, 2020, 2020-2022, 2024 Realtek Corporation; [precise references](hwdocs/rtl8852be.md) | **BSD-3-Clause** ([notice](LICENSES/BSD-3-Clause-rtw89.txt)), chosen from upstream's `GPL-2.0 OR BSD-3-Clause` offer; firmware binary has separate terms |
| `rtw89_wifi/src/{phy,phy_tables,dack,dack_tables,rxdck}.rs`, `rtw89_wifi/src/phy/{dm,tail}.rs`, `rtw89_wifi/src/calibration.rs` | RTL8852B RFE-1/cut-1 table selection, BB/RF/NCTL writes, gain decoding, chip power/reset interleaves and RCK/DACK/initial RXDCK calibration plus the contiguous initial DM register prefix/tail and board calibration decoder from pinned Linux `05f7e89ab9731565d8a62e3b5d1ec206485eeb0b`, Realtek Corporation; [source files, hashes and extraction](hwdocs/rtl8852be-phy.md) | **BSD-3-Clause** ([notice](LICENSES/BSD-3-Clause-rtw89.txt)), selected from the dual licence; no proprietary firmware |
| `libmind/src/voice/russian.rs` (`rules`), `scripts/voice_tts/dictionary.py` (`convert`) | The Russian letter-to-sound rules of [vosk-tts](https://github.com/alphacep/vosk-tts) 0.3.61 (`vosk_tts/g2p.py`, alphacep), ported; the dictionary they serve stays with the model on the model disk (`models/manifest.toml`: `tts-ru-vosk-0.7`) | Apache-2.0 |

Everything else was written for this project, including the 8×8 bitmap font in `common/font.rs`, which was drawn for MIND Core.

`rtl8168/src/identity.rs`, `rtl8168/src/rings.rs` and `scripts/hardware/rtl8168_probe.py` use hardware facts (descriptor fields, register offsets and the XID mask) cross-checked against FreeBSD releng/14.2 `sys/dev/rl/if_rlreg.h` and `sys/dev/re/if_re.c`, and Linux v6.12 `r8169_main.c`. [The RTL8168 notes](hwdocs/rtl8168.md) record the primary links. Those files copy no implementation or firmware payload; the controller adaptation is listed separately above.

## Rust dependencies (fetched by Cargo, not stored here)

| Crate | Used by | Licence |
|---|---|---|
| `uefi` 0.27, `uefi-raw`, `uefi-macros` | `bootloader` | MPL-2.0 |
| `linked_list_allocator` 0.10, `spinning_top`, `lock_api`, `scopeguard` | `kernel` | MIT OR Apache-2.0 |
| `smoltcp` 0.14 (TCP/IP), `managed` 0.8 | `netstack` | 0BSD |
| `heapless`, `hash32`, `stable_deref_trait`, `cfg-if`, `bitflags` 1.3 | `netstack` (through `smoltcp`); `cfg-if` also `tls`, `keystore`, `bootloader` and `updater` (through the crypto crates) | MIT OR Apache-2.0 |
| `byteorder` | `netstack` (through `smoltcp`) | Unlicense OR MIT |
| `rustls` 0.23 (TLS 1.3 client) | `tls` | Apache-2.0 OR ISC OR MIT |
| `rustls-webpki` 0.103, `untrusted` 0.9 | `tls` (certificate verification, through `rustls`) | **ISC** ([text](LICENSES/ISC-webpki.txt)) |
| `rustls-pki-types` | `tls` | MIT OR Apache-2.0 |
| RustCrypto: `aead`, `aes`, `aes-gcm`, `chacha20`, `chacha20poly1305`, `poly1305`, `polyval`, `ghash`, `ctr`, `cipher`, `universal-hash`, `inout`, `sha2`, `hmac`, `hkdf`, `digest`, `block-buffer`, `crypto-common`, `p256`, `p384`, `ecdsa`, `elliptic-curve`, `primeorder`, `sec1`, `rfc6979`, `crypto-bigint`, `ff`, `group`, `base16ct`, `der`, `spki`, `pkcs1`, `pkcs8`, `const-oid`, `signature`, `ed25519`, `rsa`, `zeroize`, `opaque-debug`, `cpufeatures` | `tls`, `keystore`, `ssh` (`chacha20`, `poly1305`, `sha2` and what they use: the SSH client, 351-NET-0026, in the SSH service `ssh_service`, 351-NET-0027); `bootloader` and `updater` (`sha2`, `digest`, `block-buffer`, `crypto-common`, `signature`, `ed25519`, `cpufeatures`: manifest verification, 350-UPD-0003; release verification, 351-UPD-0007) | MIT OR Apache-2.0 |
| RustCrypto: `x509-cert` 0.2, `der_derive` (build time) | `tls` (the pinned certificate's validity, purpose and CA:false, 750-NET-0020) | MIT OR Apache-2.0 |
| `flagset` 0.4 | `tls` (through `x509-cert` and `der`) | Apache-2.0 |
| RustCrypto: `sha1`, `pbkdf2`, `aes-kw`, `ccm` | `wpa` (WPA2-PSK for the 802.11 station, 550-NET-0016; through it also `wlan`, and `keystore` for the remembered networks, 550-NET-0024), with `aes`, `hmac`, `aead`, `cipher`, `ctr`, `inout`, `digest`, `block-buffer`, `crypto-common`, `cpufeatures` (above), `subtle`, `generic-array`, `typenum`, `cfg-if`, `version_check` (below), and `libc` in its host tests only | MIT OR Apache-2.0 |
| `curve25519-dalek` 4, `x25519-dalek` 2, `ed25519-dalek` 2, `subtle` 2 | `tls`, `keystore`, `ssh` (the SSH client's key exchange and host key, 351-NET-0026, in the SSH service `ssh_service`, 351-NET-0027); `bootloader` and `updater` (`curve25519-dalek`, `ed25519-dalek`, `subtle`: manifest and release verification) | **BSD-3-Clause** ([text](LICENSES/BSD-3-Clause-dalek.txt)) |
| `num-bigint-dig`, `num-integer`, `num-iter`, `num-traits`, `rand`, `rand_chacha`, `rand_core`, `ppv-lite86`, `lazy_static`, `once_cell`, `smallvec`, `typenum`, `zerocopy` (also BSD-2-Clause) | `tls` (through `rsa` and `rustls`) | MIT OR Apache-2.0 |
| `generic-array`, `spin`, `libm` | `tls` (through the RustCrypto crates and `rsa`); `generic-array` also `keystore`, `bootloader` and `updater` | MIT |
| `curve25519-dalek-derive`, `typenum` (also `tls` above); `fiat-crypto` (in the lock files, not compiled: only for the `fiat` backend; also BSD-1-Clause) | `tls`, `keystore`, `bootloader`, `updater` (through `curve25519-dalek` and `generic-array`) | MIT OR Apache-2.0 |
| `rustc_version`, `semver`, `version_check` | build time only (build scripts of `curve25519-dalek` and `generic-array`) | MIT OR Apache-2.0 |
| `ucs2` 0.3 | `bootloader` (through `uefi`) | MPL-2.0 |
| `log`, `bitflags`, `uguid`, `bit_field`, `ptr_meta` (MIT only) | `bootloader` (through `uefi`) | MIT OR Apache-2.0 |
| `syn`, `quote`, `proc-macro2`, `unicode-ident` (also Unicode-3.0) | build time only (procedural macros of `uefi`) | MIT OR Apache-2.0 |

`rsa` only verifies signatures with public keys in `tls`; the timing side channel of its private-key operations (RUSTSEC-2023-0071) does not apply. The BSD-3-Clause and ISC crates are compiled into `tls.elf` and `keystore.elf`, and the BSD-3-Clause `dalek` crates into the bootloader (`BOOTX64.EFI`, `BOOTAA64.EFI`) and `updater.elf`, so a distributed boot image carries their notices (`LICENSES/`, which `scripts/make_usb_image.py` packages whole).

MPL-2.0 is a file-level copyleft: it applies to those crates' own files, not to MIND Core. The exact versions are pinned in each `Cargo.lock`.

## Speech models (fetched, not in this repository)

The recognition and synthesis models of main tasks 250, 251 and 252 are not committed. `scripts/models.py` fetches them into a model cache and writes them to a model disk. [`models/manifest.toml`](models/manifest.toml) pins each one to a revision and records every file's SHA-256 with the model's source, licence and terms. The cache and the disk carry the same record in `MANIFEST.json`.

Any free model may be used (the maintainer, 2026-10-08), so these licences differ from MIND Core's:

- some weights are under CC BY 4.0, which requires attribution wherever they go;
- some models were trained on data licensed for non-commercial use only (for example GigaSpeech), and their `terms` say so;
- a distribution of a model disk or of an image that includes models must keep each model's licence and terms.

## Proprietary files (neither in this repository nor in any image)

Their terms forbid redistribution. A script fetches each from the source named here and checks it by SHA-256 on the maintainer's machine. It keeps them in `proprietary/`, the separate store for all such components in the working tree, which `.gitignore` excludes. After the image is written, the script copies it onto the disk, under `data/firmware/` on the boot volume. A disk that holds one is for the maintainer's own use (AGENTS.md, section 3).

| Files | Needed by | Source | Script |
|---|---|---|---|
| Broadcom's 802.11 microcode for the BCM4331: `ucode29_mimo`, `ht0initvals29`, `ht0bsinitvals29` | `bcm_wifi` from its stage 2 ([550-DRV-0006](issues/550-DRV-0006-broadcom-wifi.md)) | Broadcom's `wl` 6.30.163.46 archive. Its first home (lwfinger.com) is gone, so it is fetched from the copy distributions use (github.com/minios-linux/b43-firmware) and checked against the SHA-256 nixpkgs publishes and the MD5 SlackBuilds publishes. `b43-fwcutter` extracts the files, and each is checked by its own SHA-256 | [`scripts/proprietary.sh`](scripts/proprietary.sh) `fetch`, then `copy` after the disk is written |

### Realtek firmware kept in the same personal store

RTL8852BE's `rtw89/rtw8852b_fw-1.bin` is fetched from official `linux-firmware` commit `599764611a8ac213c6aa6dad17c941c2f46b53cb` by [`scripts/firmware/rtl8852be.py`](scripts/firmware/rtl8852be.py), also invoked by the standard `scripts/proprietary.sh fetch` used by `build-and-write.sh`. [The source/size/hash record](hwdocs/rtl8852be.md#firmware-pin-and-personal-store) pins both firmware and `LICENCE.rtlwifi_firmware.txt`. Unlike the Broadcom files above, Realtek's terms permit unmodified binary redistribution with its notice and conditions; they prohibit reverse engineering, decompilation and disassembly. The project's personal-store workflow is nevertheless used: both downloads stay under ignored `proprietary/firmware/rtw89/`, outside built images and this repository, and `scripts/proprietary.sh copy` copies them together after a disk image is written. The binary is not a test fixture or part of the BSD-licensed helper code.

## Tools used at build or test time (not distributed)

- OVMF (EDK II UEFI firmware, BSD-2-Clause-Patent) to boot QEMU; installed by the system package manager or placed next to the launch scripts.
- QEMU, Rust toolchain, Python.
- The TLA+ tools (`tla2tools.jar` 1.7.4, MIT licence, [tlaplus/tlaplus](https://github.com/tlaplus/tlaplus)) to check the models of `docs/assurance`; `scripts/model_check.sh` fetches them and checks their SHA-256; Java.
- Optional: a Vosk speech model for the `tts` test (`--asr-model`).
