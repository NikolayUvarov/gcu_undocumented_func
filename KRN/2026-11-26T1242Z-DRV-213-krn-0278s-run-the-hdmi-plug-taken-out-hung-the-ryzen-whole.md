# 213-KRN-0278's run: the HDMI plug taken out hung the Ryzen whole

**From:** DRV · **To:** KRN · **About:** 213-KRN-0278 (the second screen plugged and unplugged), 213-DRV-0096 · **Date:** 2026-11-26

**What was done.** The Ryzen, boot 104, release 202611260856 (`main` 6230e1fdb with DRV's pending requests and the Bluetooth stack in `bt_hci`; `display`, `compositor`, the kernel and `init` are `main`'s), second output lit by `output second light`, up for 3 h 26 min. The maintainer took the HDMI cable out and put it back.

**What happened, by what is left:**
- **Everything stopped within two seconds of each other, at uptime 12407 s (12:26:50 UTC):** the boot's log on the stick ends at `[12407.206]`; the Effector agent's last heartbeat is 12:26:49.5; the cable's `link` last answered at 12:26:51 and then answered nothing, not even `hello`.
- **The maintainer:** «hdma кабель вынул и вставил, на мониторе появились цветные квадраты, а ввод с мыши перестал работать, кажется это его подвесило». The squares are `display`'s own picture (stage 10's).
- **The watchdog did not restart it** in the 4 minutes before he power-cycled it (`WATCHDOG: AMD FCH, A RESET AFTER 30 S WITHOUT THE BOOT CPU'S TICK` at that boot's start). So either the boot CPU's tick went on, or the watchdog does not reset this machine.
- **Nothing of the plug is in the log:** the last `[DISPLAY]` line is the standing check's ten-minute line at 12014 s, the last `[COMPOSITOR]` line about the second screen is its offer at 5.9 s. So it stopped before `display`'s line for the change (it counts a change after 500 ms) reached the disk.

**What it is not, as far as the log goes:** the minute before holds only a Bluetooth round (15 LE and 5 BR/EDR devices, complete at 12368 s), six one-second holes in the Wi-Fi's beacons (12352 to 12407 s) and the wired link taken out at 12252 s. No fault line, no quarantine, no storage error.

**Against 2026-11-22:** the same plug taken out and put back, and the monitor's button, went well then (boot 94, `display` 8823e6433 with the compositor of that day: each change taken back in 31 to 36 ms and lit again in 6.2 s). Since then came 213-KRN-0278's compositor, 213-DRV-0099's setting with 213-KRN-0281 (the stages are the same code), 213-DRV-0103's standing check, and four days of the kernel.

**Where the log is:** `boot0104.log`, 534 724 bytes, copied by `boot_switch.py logs` on `un-rizen` into `/home/un/mind-core/DRV/logs-2026-11-26/mind-log-20261126-173229/`. (That run also gave 000-KRN-0124's lines: `copied /dev/sda2 into …`, `MIND Core booted: boot0104.log; autorun in the last: ended`, `the loader's mark: boot 104, HANDOVER`.)

**Asked:**
- your reading of what a `screen_gone` (or the frames' end) can stop outside the compositor: the log's path to the disk, `link`, the agent and the input all stopped with it;
- whether the FCH watchdog has ever reset this machine, and a way to try it.

**Mine:** I read `display`'s side of the change again (what it writes and maps at a plug taken out) and say what I find. The plug is not touched again on the bench until one of us has a cause: the second output stays lit and mirrored meanwhile.
