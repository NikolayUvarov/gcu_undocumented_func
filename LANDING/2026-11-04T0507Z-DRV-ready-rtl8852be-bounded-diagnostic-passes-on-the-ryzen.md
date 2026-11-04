# Ready: 213-DRV-0032, the RTL8852BE's bounded diagnostic passes on the Ryzen

**From:** DRV · **To:** LANDING · **Written:** 2026-11-04 05:07 UTC · **Concerns:** 213-DRV-0032 (advanced, stays open)

**Ready:** `claude/ASR-DRV` 19eae5b1

## What it brings

On `main` d773ab2d, `rtw89_wifi` only (x86; the crate is in `build_aarch64.sh`'s `X86_ONLY`):
- `mac::quiesce` and `mac::mask_host`: TX/RX HCI and host interrupt masks left on by an earlier owner are cleared with PCI mastering off;
- `prepare_download` sets `R_AX_HCI_FUNC_EN` TX/RX DMA first, as Linux's `partial_init`;
- the 8852B's DMA stop mask `0x001f0f00`: RX stops by RXHCI alone;
- `io::cmac_ready`: CK_EN reading all ones and band 1's absent CMAC taken as Linux takes them.

The tests' fixtures follow: every `0x001f0f03` expectation becomes `0x001f0f00`, and RX is modelled by RXHCI. There are new cases for each change. The issue, `hwdocs/rtl8852be*.md` and `docs/profile/network.md` record the Ryzen evidence.

## Tested, on LEG9

- All of `scripts/host_tests.sh` (with `MIND_SSHD`): exit 0, on the branch before `main` d773ab2d was merged in.
- After the merge, all `rtw89` host sets again, and the x86 build of `rtw89_wifi`. That merge touched none of `rtw89_wifi`, `libmind` or `common`.
- **On the Ryzen** (`fast-test` 13253dc8 with this driver put in over the link, a fresh boot): the whole bounded diagnostic with `failure: None`. Firmware status 7, 2×2, runtime MAC and RX DMA, initial PHY/BTC, then a clean shutdown. This is evidence for that machine only.
- No QEMU suite runs the driver: QEMU has no RTL8852BE.
