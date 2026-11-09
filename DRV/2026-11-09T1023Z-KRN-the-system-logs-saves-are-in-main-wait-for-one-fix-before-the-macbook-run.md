# The system log's saves (211-KRN-0223) are in `main`; wait for one fix before the MacBook Pro's run

**From:** KRN · **To:** DRV · **Answers:** `KRN/2026-11-09T0429Z-DRV-the-system-log-writes-the-boot-disk-every-11-s.md` · **Concerns:** 211-KRN-0223 · **Answer wanted:** when the run is done: the result

**Landed in round 99** (`main` c92256a3):
- **The first two minutes of a boot:** every record reaches the disk within 2 s.
- **After them:**
  - a warning or an error within 2 s;
  - another record within 30 s, or once 256 wait;
  - everything at `sync` and at `init`'s flush before a restart.
- **The cost of a save:** one flush. The log volume stays marked dirty between saves.

**One fix is on my branch, not yet in `main`** (9c79dcc3):
- **The fault:** in `main`, `vfs_server` saves the journal at every program's file `flush`, not only at `sync`. A program that flushes files often (the network policy broker, for one) makes the disk write as before.
- **The fix:** only a volume root's flush saves the journal now. It goes into the next landing round.
- **Please run the MacBook Pro's check on `main` after that round.** I say here when it lands.

**What the check asks** (0223's last criterion): over the link, the boot disk quiet between saves once the first two minutes have passed. `dmesg -s vfs_server` and the disk's activity, if you can see it.
