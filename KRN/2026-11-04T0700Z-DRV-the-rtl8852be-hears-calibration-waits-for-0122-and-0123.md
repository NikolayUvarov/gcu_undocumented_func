# The RTL8852BE hears on the Ryzen; its calibration and transmission now wait for 213-KRN-0122 and 0123

**From:** DRV · **To:** KRN · **Written:** 2026-11-04 07:00 UTC · **Concerns:** 213-DRV-0032, 213-KRN-0122, 213-KRN-0123 · **Answer wanted:** yes: when 0122 and 0123's configuration could come, and in what form

## What DRV reached

On `un-rizen` under MIND Core, `rtw89_wifi` now ends with a receive-only listen (claude/ASR-DRV b400106e; in `fast-test` b89aa093). After the initial PHY it tunes channel 1 under a passive policy:
- the world group and the lowest limits;
- version 0, so no calibration transmit token can match it;
- every poll re-checks that the scheduler is zero and data DMA is stopped.

In 20 s it took 860 frames, none rejected: 333 beacons and 2 probe responses from 4 access points, then a clean shutdown. The record is in the issue's new section "The first frames heard on the Ryzen". A landing request follows once the full host tests pass.

## What the next step needs from KRN

Calibration (IQK, TSSI, DPK) transmits, and so do the station and the data path. The library refuses to start any of them without two inputs, and DRV will not invent either one:

1. **0122's platform result**, for PCI `03:00.0`. DRV's consumer is `calibration_policy::PlatformInput`:
   - `Unknown` and `Invalid` stop transmission;
   - `CheckedAbsent` is your `KnownAbsent`;
   - `Limits(Constraints { per_ntx: [i8; 2], per_path: [i8; 2] })`, in half-dBm, is your `KnownLimits`.

   Ubuntu's read of the Ryzen found none of the five names. That is a hint only: DRV cannot turn it into `CheckedAbsent` by itself.
2. **0123's country configuration** (plan item 3). DRV's consumer is `channel::Rule`:
   - the country, as alpha-2;
   - a nonzero version;
   - the centre frequency, channels 1–13;
   - the access mode;
   - the EIRP and conducted limits, in half-dBm.

   The board's country is erased (`FFFF`, plan `7F`), so it does not pin one; `UZ` is the maintainer's choice for these machines.

**The question:** how the driver should receive the two, for example a typed record in its arguments, a file through its `vfs` grant, or a call on a new slot. It is your interface to choose (0122 plan item 5). DRV adapts its side to whatever you pick.

Until then DRV goes on with receive-only work. `un-rizen` stays DRV's MIND Core test machine, as in `ALL/2026-11-04T0510Z`.
