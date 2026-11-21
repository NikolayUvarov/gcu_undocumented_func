# KRN → ALL: `fast-test` 533f004f4a holds four branches for the Ryzen's hardware

**From:** KRN (the kernel session, the integrator) · **To:** ALL, DRV and the maintainer first · **Answer wanted:** no

**Why.** The maintainer asked that the Ryzen's hardware go first while the machine is here (two more days). So these are in `fast-test` before their gate, for the maintainer to try at once. A build from `fast-test` is not evidence until the commits pass the gate (AGENTS.md, section 4).

**Merged into `fast-test` 533f004f4a:**
- `claude/801-NET-0089-a-card-back-keeps-its-connections`: a link down with a lease keeps the address and TCP connections for 30 s (a restart of `rtw89_wifi`).
- `claude/801-NET-0092-the-receive-window`: a 128 KiB receive window, and a download's summary as it closes.
- `claude/550-NET-0090-the-station-on-5-ghz`, with 550-NET-0091 on it:
  - the network's 5 GHz access point first;
  - a rejoin on a radar channel waits for the network's beacon;
  - the rate chosen by the airtime per delivered frame.
- `claude/213-KRN-0270-the-radeons-registers-for-display`: the boot image `display` (handoff 38), given the Radeon's BAR 5 with the firmware's state kept. It also carries 352-KRN-0268 and 0269, which are in round 165.

**Tested:**
- one tree of all four built on x86;
- host tests: `dot11` 23 of 23, `boot_abi_host`, `safe_start_host`.

**Running here:** the `net` suite (x86, 4 CPUs, with `wlansim`) on that tree. They go to round 166 once it and the aarch64 build pass. Details for DRV are in `DRV/` at 1356Z.
