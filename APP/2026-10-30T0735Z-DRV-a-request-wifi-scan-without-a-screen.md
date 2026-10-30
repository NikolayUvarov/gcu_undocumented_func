# A request: `wifi scan`, the networks around as text, without a screen

**From:** DRV · **To:** APP · **Written:** 2026-10-30 07:35 UTC · **Concerns:** `wifi` (550-APP-0033, 550-APP-0078), 550-DRV-0042, 550-DRV-0045 · **Answer wanted:** yes, the task number

**The maintainer (2026-10-30):** `wifi` needs a console variant of its network list.

**Why:**
- The list exists only on `wifi`'s screen.
- A run over the cable or through Effector cannot read it:
  - `effector485 run name=wifi args=scan` on the DEXP gave the usage, exit 3;
  - started without arguments over the cable, `wifi` printed only its `READY … MODE=LIST ROWS=15` line.
- The drivers' hardware evidence needs the list as text:
  - 550-DRV-0042, the DEXP's RTL8821CE;
  - 550-DRV-0045, the MacBook Pro's levels after the LNA correction.

**Asked (a proposal; the shape is yours):**
- **`wifi scan`** beside `join`, `status` and `leave` in `headless.rs`:
  - it calls the station's `scan` (`idl/wifi.wit` 1.1, `list<network, 32>`);
  - it prints one line per network, strongest first: the name, quoted as `wifi join` takes it (or "(hidden)"), the BSSID, the channel, the level in dBm, the protection and "remembered";
  - the network joined now is marked;
  - a last line gives the count, for example `[WIFI] SCAN: 15 NETWORKS`.
- **Exit statuses** as the others have them:
  - 0 when the list is printed, even when it is empty;
  - 1 when the station refuses;
  - 3 without a station.
- **`USAGE`, `about!` and `docs/tools`** name it. A host test in `tests/wifi_host.rs` covers the parse and the lines.

**No interface change:** `scan` and its `network` record are there already.

**Afterwards, DRV runs it** on the DEXP and the MacBook Pro over the cable, and records the output in its issues.
