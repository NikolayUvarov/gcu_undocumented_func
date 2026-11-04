# Ready: 213-DRV-0032, the RTL8852BE's receive-only listen hears the first frames on the Ryzen

**From:** DRV · **To:** LANDING · **Written:** 2026-11-04 07:10 UTC · **Concerns:** 213-DRV-0032 (advanced, stays open)

**Ready:** `claude/ASR-DRV` cf37c268

It replaces `LANDING/2026-11-04T0509Z-…` (ee590538), which was not answered yet. It holds those commits and one more, b400106e, merged with `main` c01928f7.

## What it brings

Everything in 0509Z: the quiescence, `HCI_FUNC_EN`, the `0x001f0f00` stop mask and `cmac_ready`. b400106e adds, in `rtw89_wifi` only (x86; the crate is in `build_aarch64.sh`'s `X86_ONLY`):
- `btc_session::listen`, now the driver's last stage. After the initial PHY it tunes one channel under `Policy::passive`:
  - `PassiveOnly`, the world group, the lowest limits and version 0, so no calibration token can match it;
  - the report tail;
  - mac80211's scan filter;
  - a bounded listen of beacons and probe responses. Every poll re-checks that the scheduler is zero and data DMA is stopped.
- `Session::observe_events`, for BTC events during a listen without commands.
- Three readbacks the chip does not give, narrowed to what it keeps; Linux reads none of them back:
  - RF `0xcf`'s trigger;
  - RF18's direct copy, now checked by its CH field, in the channel stage and in data mode's check;
  - the MAC power tables, now compared by seven bits a byte.

The host fixture models all three. New cases in `tests/rtw89_listen_cases.rs`, run by `rtw89_initial_phy_host.rs`. The issue, `hwdocs/rtl8852be.md` and `docs/profile/network.md` record the evidence.

## Tested, on LEG9

- All of `scripts/host_tests.sh` (with `MIND_SSHD`) on b400106e: exit 0.
- `main` c01928f7, merged in after that run, changed only `issues/`.
- **On the Ryzen** (`fast-test` 13253dc8 with this driver put in over the link, a fresh boot, build a098ef06, which matches b400106e's build): channel 1 for 20 s. It took 860 frames, none rejected, among them 333 beacons and 2 probe responses from 4 access points, then a clean shutdown. This is evidence for that machine only.
- No QEMU suite runs the driver: QEMU has no RTL8852BE.
