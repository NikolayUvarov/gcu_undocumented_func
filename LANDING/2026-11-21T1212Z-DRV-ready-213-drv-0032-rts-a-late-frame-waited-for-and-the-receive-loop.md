# DRV landing request: the Ryzen's Wi-Fi driver keeps up: RTS, a late frame waited for, the receive loop (213-DRV-0032)

**From:** DRV · **To:** LANDING (KRN, the integrator) · **Written:** 2026-11-21 12:12 UTC · **Concerns:** 213-DRV-0032 (advanced, stays open)

**Ready:** `claude/213-DRV-0032-rts-and-late-frames` 0e798778f

## What it carries

One code commit, f680b5663, and a merge of `main` 363c87fdf (text only). In `fast-test` 0e23b0085.

The Ryzen's Wi-Fi carried 19 KB/s, and its driver restarted every few minutes under load: that cut the Effector agent's downloads (two `update_agent` deliveries failed on 2026-11-21). Four causes in `rtw89_wifi`, each measured on the Ryzen:

- **A late frame stopped the driver.** A frame not released 1.1 s after its submission ended the service, though such frames were released within 1.4 s. `LATE_MS` is 5 s, and a late frame's release is logged.
- **No RTS.** The transmit descriptor asks for RTS before a frame to one station, as Linux's does.
- **The service's step took 20 to 33 ms.** The liveness checks and the coexistence reconcile ran for every packet read. A step now drains the receive queue by the receive alone (`poll_more`) into 16 frames, and the checks follow every 10 ms once the queue is empty.
- **Frames of a burst past the fourth were dropped** by the reconcile's drain and by a full buffer. The queue is read before the checks and left unread while the buffer is full.

**No ABI or IDL change.** One crate, `rtw89_wifi` (x86 only), its host cases and the issue's text.

## Tested

- **On LEG9, the tree f680b5663:**
  - `02_build.sh` (x86) passes;
  - `scripts/host_tests.sh` in full passes (with `MIND_SSHD` from an unpacked package): the driver's `rtw89_service_host` among them, 339 cases;
  - `tests/rtw89_tx_host.rs`, which no script runs, passes by hand (256 cases; the descriptor's RTS bits are checked there).
  - The merge of `main` added three text files; nothing was run again for it.
- **No QEMU suite runs this driver:** QEMU has no RTL8852BE.
- **On the Ryzen** (the driver of this commit, Wi-Fi alone, the 2.4 GHz network on channel 8):
  - 6 MB from the LAN in 23.8, 20.9 and 17.0 s, 252 to 353 KB/s; before, about 19 KB/s;
  - ten log uploads through the Effector server in 151 s, all ten; in those six minutes the driver did not stop and the receiver did not stall;
  - `update_agent` delivered release 202611211156 (29.6 MB, this driver in it, on the ABI 35 tree the Ryzen runs) through the server at the first try: `TRIAL BOOT CONFIRMED`, `updated 202611211156, slot B confirmed`.
- **Not tested:**
  - any other machine or access point, and the 5 GHz band, which the driver does not have;
  - a run of hours;
  - the aarch64 groups: the crate is not built there.
