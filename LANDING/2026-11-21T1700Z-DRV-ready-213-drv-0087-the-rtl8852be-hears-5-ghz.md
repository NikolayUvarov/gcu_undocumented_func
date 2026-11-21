# DRV landing request: `rtw89_wifi` on 5 GHz: the channels, their rules, the calibrations' branches; heard on the Ryzen, nothing sent there yet (213-DRV-0087)

**From:** DRV · **To:** LANDING (KRN, the integrator) · **Written:** 2026-11-21 17:00 UTC · **Concerns:** 213-DRV-0087 (advanced, stays open)

**Ready:** `claude/213-DRV-0087-rtl8852be-on-5-ghz` 863735122

## What it carries

On `main` e703fccc8 (merged in, ABI 37). In `fast-test` d55d9e4ba.

- **5 GHz in the driver** (213-DRV-0087), 20 MHz channels at 6 to 54 Mb/s:
  - a band beside the channel: 1 to 13 and Linux's 28 channels of 5 GHz;
  - the power tables and their generator, extended from the same pinned sources (`--check` reproduces them);
  - the country's rules: DFS and NO-IR channels listen-only, channels with an indoor or outdoor restriction left out;
  - the channel setting's, IQK's, TSSI's and DPK's 5 GHz branches, and the coexistence's (as Linux's 5 GHz link mode);
  - `info`: `bands` 7, 41 channels in `supported`;
  - **the listen-only rule:** on such a channel nothing is admitted or sent until the access point's beacon was heard there since the tune, a probe request never, and in data mode sends stop after 5 s without its beacon.
- **A parser for a frame's signal** from the hardware's PPDU status report (`ppdu.rs`), with host cases on real reports. It is not wired into the receive header yet: byte 2 stays 0, as in `main`.
- **No ABI or IDL change.** `rtw89_wifi` (x86 only), its generators, tests and notes.

## What a machine with this driver does differently on 2.4 GHz

- **A scan takes about 4 s longer** while the country bars the 5 GHz channels: `dot11` dwells 150 ms on each barred channel. On the Ryzen under RU, 5.6 s against 1.35 s. NET is asked to skip barred channels (`NET/2026-11-21T1700Z-DRV-request-a-scan-skips-the-channels-the-country-bars.md`).
- The first, listening scan visits all 41 channels: 6.3 s.
- The join and the rate on channel 8 are as before.

## Tested

- **On LEG9, the tree 863735122:** `02_build.sh` (x86) passes; all 40 `rtw89_*_host.rs` pass; the full `scripts/host_tests.sh` passes (254 suites, with `MIND_SSHD`); the generators' `--check` reproduce the tables.
- **On the Ryzen** (release 202611211156, ABI 35, with the branch before `main` was merged in, 90699adaa and bf441efca):
  - a listening scan tuned all 41 channels, the longest tune 4.3 ms, none failed, and heard the bench's access point on channel 100 and seven others on 36 to 157;
  - the station then joined on channel 8; 6 MB from the LAN at 289, 292 and 303 KB/s.
- **No QEMU suite runs this driver:** QEMU has no RTL8852BE.
- **Not tested:**
  - **anything sent on 5 GHz:** the calibration there, a join, a download. Under the country the bench runs under, no 5 GHz channel may be sent on; the maintainer is asked for the country and a channel;
  - the listen-only rule and the 5 GHz coexistence on hardware (host cases only);
  - the tree with `main` merged in on the Ryzen: its ABI 37 does not run on the release there until the stick is written anew;
  - the aarch64 groups: the crate is not built there.
