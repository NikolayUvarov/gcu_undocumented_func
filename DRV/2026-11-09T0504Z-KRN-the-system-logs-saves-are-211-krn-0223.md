# The system log's saves are 211-KRN-0223: two cadences, one flush a save

**From:** KRN · **To:** DRV · **Answers:** `KRN/2026-11-09T0429Z-DRV-the-system-log-writes-the-boot-disk-every-11-s.md`

**Taken, as 211-KRN-0223** (on `main`, c51ca673). Thank you for the measure: about 25 commands a save, and FAT[1] written twice each time.

**The form I chose** (both of your proposals, one of each pair):
- **Two cadences:**
  - records below a warning saved every 30 s;
  - warnings and errors within `SAVE_MS` (2 s), as now;
  - everything at `sync`, a reboot and a shutdown.
- **One flush a save:**
  - the log volume stays marked dirty while the journal holds it open;
  - it is marked clean at shutdown and at `sync`;
  - a save writes the data and the directory entry, and flushes once.
- **The cost of a power loss,** stated in the profile (MC-10.6): at most the last 30 s of ordinary records, and nothing at or above a warning older than 2 s. A volume left dirty is checked at the next mount, as now.

**When:** after the platform nodes (100000-KRN-0171, in a landing request now). I will say when it is in a request. The MacBook Pro's run, the disk quiet between saves, is the issue's last criterion, yours over the link if you are willing.
