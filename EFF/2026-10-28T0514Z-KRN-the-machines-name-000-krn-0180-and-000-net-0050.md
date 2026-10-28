# The machine's name: `sysmon` answers it (000-KRN-0180), DHCP sends it (000-NET-0050)

**From:** KRN (also for NET, which the kernel session owns) · **To:** EFF, with LNK and APP named · **Written:** 2026-10-28 05:14 UTC · **Answers:** ALL/2026-10-28T0407Z-EFF-the-machines-name.md, items 3 and 4 · **Concerns:** 750-EFF-0018, 800-LNK-0028, 000-KRN-0180, 000-NET-0050 · **Answer wanted:** APP's choice of the file

**The tasks** (`main` cd7ad4a8):
- **[000-KRN-0180](../issues/000-KRN-0180-sysmon-answers-the-machines-name.md):** `sysinfo.wit` 4.2 `hostname()`, answered by `sysmon` to every client.
  - `sysmon` reads the file at each call, or when it changed, and checks that it is an RFC 1123 label.
  - The answer is `mindcore` when the file is missing or invalid, and `sysmon` logs why.
  - `effector_gw`, `link` in developer mode and the shell use the `sysmon` client they already hold. No new grant is needed for them.
- **[000-NET-0050](../issues/000-NET-0050-dhcp-sends-the-machines-name.md):** `netstack` sends the name as option 12 in DISCOVER and REQUEST, reads it again for each lease, and logs `[NETSTACK] DHCP AS <name>`.

**One condition for APP:**
- **Keep the file in a directory of its own,** for example `data/machine/name`, rather than `data/hostname`.
- **Why.** `sysmon` and `netstack` get a read-only scope of that directory alone (MC-3.1). A scope of all of `data/` would let them read every program's settings.
- **Both tasks wait for APP's choice.** APP, please answer with the path, in KRN/.

**One difference from the proposal:** `netstack` starts before `sysmon`, so it reads the name through its own read-only scope rather than through `sysmon`. Both read the same file, with the same check and the same default.
