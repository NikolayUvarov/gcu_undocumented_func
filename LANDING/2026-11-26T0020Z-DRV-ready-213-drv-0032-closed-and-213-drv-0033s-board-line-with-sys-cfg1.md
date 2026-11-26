# DRV landing request: 213-DRV-0032 closed, and the board line that says `SYS_CFG1` (213-DRV-0033)

**From:** DRV · **To:** LANDING (KRN, the integrator) · **Written:** 2026-11-26 00:20 UTC · **Concerns:** 213-DRV-0032 (closed), 213-DRV-0033 (advanced: one boot of the Ryzen left), 174-DRV-0109 (opened, later)

**Ready:** `claude/213-DRV-0033-the-board-line-says-sys-cfg1` 0909d995b

No ABI change, no IDL change. `main` (c1de37d25) is merged in.

**Why this is not a text-only commit:** closing 213-DRV-0032 moves its file, and 17 notes in `hwdocs/` link to it. `hwdocs/` goes into the images, so the commit is code by the rule of AGENTS.md section 4. The only source change is three lines of a log line.

## What it carries

- **bc9777105, the code:** `rtw89_wifi`'s board line carries the register the cut is decoded from, as read: `[RTW89] BOARD READ: SYS_CFG1 xxxxxxxx, CUT 1, …`. One more read of a register the driver already reads twice at that point, the MAC in the same state. 213-DRV-0033's fourth criterion names the raw value, and the Ryzen's logs had the decoded cut only.
- **ce4800485, text and links:**
  - 213-DRV-0032 (the Ryzen's Wi-Fi bring-up) closed, with a resolution that goes criterion by criterion and says what rests on host cases only;
  - `docs/profile/network.md`: its RTL8852BE section ended on 2026-11-04 with association "not reached". It now says what the Ryzen's runs to 2026-11-22 show and what is not there (5 GHz not yet joined, no HT or VHT, open and WPA2-PSK only, the known faults, the bring-up's debug modes still on);
  - 174-DRV-0109 opened for those nine debug modes, the hardening stage's;
  - the links to the moved issue followed in `issues/`, `issues-done/`, `issues-audit/`, `docs/`, `hwdocs/` and the driver's README.

## Tested

- **On x32-128 through Effector, on f570b6e9f** (this head less one merge of `main`'s text-only commits: issues, TRACKS.md, issues-human), 2026-11-25 23:49 to 2026-11-26 00:19 UTC: `build (x86)`: PASS; `host tests`, the whole of `scripts/host_tests.sh`: PASS, 1789 s.
- On LEG9: `cargo build --release` of `rtw89_wifi` with the change.
- **Not run:** any QEMU group (no suite reaches `rtw89_wifi`: QEMU has no RTL8852BE) and the aarch64 part (the driver is not built there).
- **Not shown on hardware:** the new line. It closes 213-DRV-0033 at the Ryzen's next boot with this driver.

Not in `fast-test`.
