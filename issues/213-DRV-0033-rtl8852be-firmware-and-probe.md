# 213-DRV-0033 — Pin RTL8852BE firmware and identify the chip before radio initialization

**Type:** firmware format / hardware evidence · **Owner:** ASTRA auditor-fixer, for `DRV`, with a `KRN` read-only probe · **Priority:** P1 · **Status:** in progress (Linux chip identity collected; MIND raw register observation pending) · **Blocked by:** the Ryzen MIND `SYS_CFG1` report · **Main task:** [213](213-amd-ryzen-pc.md), [550](550-network-on-real-hardware.md) · **Roadmap:** tracks H, A · **Constitution:** MC-12.1, MC-12.3

## Problem

The PCI ID `10ec:b852` establishes the adapter family, but not its silicon cut, RFE type or calibration. The official `rtw8852b_fw-1.bin` contains several images; selecting the wrong variant or trusting unchecked sizes would make later firmware download unsafe. No firmware binary may enter the repository or a built image.

## Plan

1. Pin Linux v6.19's dual-licensed rtw89 public format/register definitions under BSD-3-Clause and the matching official `linux-firmware` revision. Keep source/binary licences distinct.
2. Fetch/check the firmware and its licence by SHA-256 into `proprietary/firmware/rtw89/`. Preserve the existing explicit post-image copying workflow; never add firmware to image construction.
3. Add `rtw89_wifi/src/firmware.rs`: bounded MFW selection by measured cut and image type, AX header-v0 parsing, section/overlap/length checks and explicit rejection of unsupported security layouts. Use synthetic test fixtures in the repository.
4. Add pure register identification helpers. KRN's hardware-report path may read `BAR2 + 0xF0` (`SYS_CFG1`) after validating the PCI function and memory BAR; no bus mastering, RF writes or indirect XTAL/EFUSE transactions in this probe. Preserve any PCI configuration state changed to permit the read.
5. Record actual raw `SYS_CFG1` and cut on the Ryzen, then select and validate that cut's normal-CE image from the verified file. Record which power/EFUSE/calibration dependencies still block the driver.
6. When Linux is available, collect existing driver/kernel/firmware/RFE observations using `scripts/hardware/rtl8852be_probe.py`. Linux 7.0 emits the digital cut and RFE in a numeric `chip info` boot message; reading it is passive. Missing or conflicting log evidence remains unresolved: never substitute PCI revision or access live BAR MMIO. `btc_info` changes firmware reporting even on read, and mac80211 exposes no register bytes. Boot-log evidence does not replace the raw-register requirement in step 5.

## Acceptance criteria

- Official firmware revision, size and SHA-256, source commit and separate licence terms are documented; fetching verifies before atomic replacement and leaves no binary outside the ignored store.
- Synthetic tests reject every truncated prefix, oversized counts/sections, overlapping or header-aliasing image ranges, duplicate selection keys, malformed dynamic lengths, unsupported security pools and future header versions. Selection does not silently use manufacturing/WoWLAN or a newer cut.
- Both available CE variants of the pinned personal firmware pass a separate explicit local validation; those bytes are not committed test fixtures.
- The actual Ryzen report records `SYS_CFG1`, a usable digital cut or a precise read failure, and the compatible CE image selection. No successful-radio claim follows from the probe.
- Radio bring-up dependencies are captured in [213-DRV-0032](213-DRV-0032-rtl8852be-wifi.md); licence notices and project host-test integration accompany the change.

## Related

[213-DRV-0032](213-DRV-0032-rtl8852be-wifi.md), [RTL8852BE source and format notes](../hwdocs/rtl8852be.md), [THIRD_PARTY.md](../THIRD_PARTY.md), `rtw89_wifi/src/`, `scripts/firmware/rtl8852be.py`, `tests/rtw89_firmware_host.rs`, `tests/rtl8852be_fetch_test.py`.

## Progress (2026-10-15)

Twelve Rust host tests pass, covering hostile/truncated synthetic inputs and pure register decoding; five offline fetcher tests pass, including bounded reads and preservation of an existing file on an invalid download. The helper library builds as `no_std`. A separately invoked proprietary-fixture test passes for both CE images (cuts 1 and 2) in the pinned firmware, version 0.29.29.15, header version 0, three sections. Cut 0 has no compatible CE image in that file and is rejected. These are host format checks, not firmware-download or radio evidence.

At 2026-10-15 18:34:35 UTC, Effector operation `20261015183435.268000692_000025` on the Ubuntu Ryzen returned the existing `rtw89_8852be 0000:03:00.0` boot record: `CID: 0, CV: 1, AID: 0, ACV: 1, RFE: 1`. Kernel `7.0.0-38-generic` has the ordinary driver bound to `wlp3s0`; the preceding identity operations observed D0/runtime active and carrier 1. The measured digital cut is 1 (CBV), with RFE 1. This came from the passive journal, without debugfs/MMIO or a radio-state change. The pinned file's CE cut-1 image is selected at offset 96, length 326616, version 0.29.29.15, commit field `0x6fb3ec41`; its hash and explicit proprietary-file parser test were rechecked after the observation. Ubuntu's separately observed running firmware is 0.29.29.5 (`da87cccd`); it is not the pinned MIND file.

The collector now uses the valid `journalctl -k` option and recognizes both legacy RFE records and the Linux 7.0 numeric chip-identity message. Fourteen tests pass, including a real `journalctl` CLI check against an empty temporary journal directory. The host test requires `journalctl` to be installed. Verified, unambiguous chip identity returns `identity-collected`/exit 0; missing/conflicting evidence returns `partial`/exit 1. SSIDs and unrelated text are excluded; it neither reads debugfs nor maps MMIO. See [usage, provenance and limitations](../hwdocs/rtl8852be.md#linux-identity-collector).

The original raw `SYS_CFG1` acceptance criterion remains pending: the Linux log gives the cut but not that register's raw value. The guarded KRN probe has not run on this Ryzen in MIND. This issue stays open; Linux identity does not establish MIND radio operation.

The corrected collector was then run on the same Ryzen by Effector operation
`20261015183831.617344254_000027` at 18:38:32 UTC. It returned
`identity-collected`, exit 0, cut 1 and RFE 1, with no warnings. Source SHA-256:
`704925aa0b2a362d54c4a088093b443799306731a437e5b6a6526599e2844b3a`.
The private report remains under the ASR evidence directory on that machine.
