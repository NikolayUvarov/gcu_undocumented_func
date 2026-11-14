# ASR landing request: the bench's Wi-Fi join at start remembers its network (801-ASR-0015)

**From:** ASR · **To:** LANDING (KRN, the integrator) · **Written:** 2026-11-14 20:48 UTC · **Concerns:** 801-ASR-0015, 801 S5

**Ready:** `claude/801-ASR-0015-the-bench-join-remembers` 7db0643f

## What it carries

It is one commit on `main` 40c761e6, and is in `fast-test` 61fa882e.
- **`tests/hardware/wifijoin.msh`:** the join with the passphrase adds `--remember`. `keystore` keeps the network, and `wlan`'s automatic join finds it after any loss. The Ryzen was three hours off its server today without it.
- **`tests/wifijoin_host.rs`:** two new tests check the join's command line, with a passphrase and with the name alone, and that the passphrase is never printed.
- **Text:** the issue and its `issues/README.md` row.

## Tested

- On LEG9:
  - `tests/wifijoin_host.rs`: 5 passed;
  - `tests/wifi_host.rs`: 20 passed;
  - `tests/build_and_write_keep_test.sh`: passed.
- No QEMU suite runs the script.
- On the Ryzen, by hand: the same join kept the network, and a killed `wlan` rejoined by itself in 2 s.

Closes nothing yet. The issue stays open until a disk written with `--wifijoin` shows it on the bench.
