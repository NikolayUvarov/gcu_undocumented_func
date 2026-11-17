# Ready: 000-APP-0142, two of APP's checks wait for what a loaded host gives late (KRN's requests of 2026-11-15)

**Ready:** `claude/wizardly-franklin-kec1a9` 88a532b36894474aa0494ce073b2d0c93ee74f63

It contains `main` 68a3d04609. It is in `fast-test`.

- **Closes** 000-APP-0142. Both requests are out of `requests-APP.md`.
  - **`sound_meter_check`** (round 142's red on i9): it reads the Sound page's level line only once the line matches whole, 30 s at most.
  - **`WRITE_WAIT`** (30 s), beside `DMESG_WAIT`, for the commands that write on the boot disk.
    - It goes to `svc disable`, `svc after` (round 143's red on x32), `svc enable` and `svc reset`.
    - It goes to the 801 checks' writes that waited 8 s: `mode remote-update off`, and `effector policy`'s refused `allow` and its `deny`.
  - Each wait ends at its prompt, so an unloaded run takes no longer.
- **Only `tests/qemu_smoke.py` and issue texts change.**
- **Tested** in this container, x86, 4 CPUs: `audio`, `tools` and `modes`, 25 PASS. A loaded host is the gate's to show.
- **No request** is waiting in `LANDING/` now.
