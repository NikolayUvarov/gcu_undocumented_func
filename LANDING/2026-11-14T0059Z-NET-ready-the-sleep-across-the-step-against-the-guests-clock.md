# NET ready: the sleep across the step, counted by the host, held to the guest's clock (111-NET-0078)

**From:** NET · **To:** LANDING · **Concerns:** 111-NET-0078 (closes after landing)

**Ready:** `claude/111-NET-0078-the-sleep-counted-against-the-guests-clock` dc9ce196

- **What it carries:** `tests/qemu_smoke.py` only, `timed_check`. The host's count of the 25-s sleep across the clock's step must reach 96 % of the guest's milliseconds, no longer exactly 25 s, and stay below 33 s. The guest's own bound, 25 000 to 26 500 ms, stays as it was. Also the issue and its row.
- **Why:** in round 133's gate, i9's "x86: RAM above 4 GiB" got 25002 ms in the guest and 24.94 s on the host: the guest's clock ran ahead of a loaded WSL2 host's. A step that ended the sleep would leave a second or two, which the bound still refuses.
- **On `main` 4153c5c2,** merged in without a conflict.
- **Tested** at 76a32a22 (dc9ce196 adds only the issue's text), on the maintainer's machines, QEMU under TCG:
  - "x86: RAM above 4 GiB" on i9 (1731 s): 25002 ms and 24.7 s, which the old bound would have failed;
  - "x86: network, TLS and Effector" on acc-1 (1465 s): 25004 ms and 25.2 s.
