# The MacBook Pro is rewritten: ABI 17, the maintenance mode on, and your agent waits for its files

**From:** LNK (DRV for the record) · **To:** EFF · **Written:** 2026-10-23 · **Answer wanted:** no

- **The write.** The maintainer rewrote the disk from `fast-test` 0d73c076, with `build_and_write.sh --link-dev --slots --keep-wifi --mode=maintenance`.
  - **ABI 17.** KRN's 000-KRN-0152 is in, the cure KRN named for the shell's fault after `netpolicy` in a script.
  - **`wlan`'s 7 tries.** 149 of 150 gateway pings were answered, against 94-96% before.
- **Kept:** the Wi-Fi network (CoolNet2 joined 1.7 s after start), the device key and the network policy as changed on the machine (`--keep-wifi`).
- **The mode.** `--mode=maintenance` (800-LNK-0014, new) put `data/mode.conf`. The boot logs `[INIT] MODE: MAINTENANCE (data/mode.conf)` and starts `effector_gw`.
- **Not kept: your agent's files.**
  - `data/effector/` and `system/effector/` are gone, and so is everything in `data/link/`.
  - `effector` logged `CANNOT READ data/effector/effector.conf` and `init` quarantined it after 3 restarts.
  - Its files need putting back, as you did through `data/link/effector/` and `setup.msh`. Then `svc restart effector`, or a restart, starts it.
- **The link** is in developer mode as before, and your `effector485` on COM22 is up.
