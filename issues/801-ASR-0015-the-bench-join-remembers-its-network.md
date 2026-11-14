# 801-ASR-0015 — The bench's Wi-Fi join at start remembers its network, so the station rejoins after a loss

**Type:** assessment (bench scripts) · **Owner:** `ASR` · **Priority:** P1 · **Status:** open (2026-11-14) · **Blocked by:** — · **Main task:** [801](801-the-bench-stays-manageable.md) (S5) · **Constitution:** MC-11.9, MC-12.1

## Problem

On 2026-11-14 the Ryzen was off its Effector server from 17:28 to 20:41 UTC.
- **17:28 UTC (1628 s after boot):** rtw89 restarted after `RECEIVE STALLED`. `wlan` saw the restart (`THE RADIO RESTARTED (IT HAS NO CHANNEL SET …)`), applied the country and stayed `Off` for over three hours.
- **No rejoin was possible.** The machine had joined at start only through `tests/hardware/wifijoin.msh`, the autorun that `build-and-write.sh --wifijoin` puts on the disk. With a passphrase in `data/wifijoin.txt`, it runs `wifi join "<name>" --passphrase-file data/wifijoin.txt` without `--remember`, so `keystore` kept no network.
- **The automatic join stays silent with nothing remembered** (`wlan/src/main.rs`, `remembered(..).is_empty()`). A fresh `wlan` (`kill 27`) did the same.

The DEXP joins the same way, with nothing remembered. The MacBook Pro does not: its `keystore` keeps the network, and it joins by itself at start (`AUTO-JOIN CoolNet2: JOINED (TRY 1)`).

## Plan

- `wifijoin.msh` joins with the passphrase and `--remember`. `keystore` then keeps the network under the device key, and the station's automatic join finds it after any loss.
- `tests/wifijoin_host.rs` checks the join's command line, with a passphrase and with the name alone, and that the passphrase is never printed.

## Acceptance criteria

- The host test passes.
- On the bench, a disk written with `--wifijoin` logs `[KEYSTORE] WI-FI NETWORK <name> KEPT`. After `kill` of `wlan`, the station logs `AUTO-JOIN <name>: JOINED` and the agent connects again.

## Progress

- **2026-11-14, the Ryzen by hand over the cable:**
  - `wifi join "CoolNet2" --passphrase-file data/wifijoin.txt --remember` logged `WI-FI NETWORK CoolNet2 KEPT FOR PID 121 (1 KEPT)`;
  - after `kill 121`, `wlan` logged `AUTO-JOIN CoolNet2: JOINED (TRY 2)` 2 s after its restart;
  - the agent logged `CONNECTED TO hesere.uz` 2 s later.
- **The script's change** is on `claude/801-ASR-0015-the-bench-join-remembers`. `tests/wifijoin_host.rs`: 5 passed.

## Related

[550-ASR-0007](550-ASR-0007-wifi-join-tests.md), [801-LNK-0046](801-LNK-0046-each-machines-settings-kept-onto-any-new-disk.md), [801-DRV-0071](801-DRV-0071-the-bench-radios-keep-their-network-across-releases.md), 801-NET-0083.
