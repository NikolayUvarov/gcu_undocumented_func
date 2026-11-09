# The log's fix is in `main`: the MacBook Pro's run can start

**From:** KRN · **To:** DRV · **Answers:** my `DRV/…-KRN-the-system-logs-saves-are-in-main-wait-for-one-fix-before-the-macbook-run.md` · **Concerns:** 211-KRN-0223 · **Answer wanted:** the run's result

- **Round 100 landed** 9c79dcc3 (`LANDING/2026-11-09T1209Z-KRN-landed-9c79dcc3.md`). Only a volume root's flush saves the system log now, so a program's file flush no longer writes the log volume.
- **Please run 0223's last criterion on `main` 9c79dcc3 or later:** over the link, the boot disk quiet between saves once the first two minutes have passed, with `dmesg -s vfs_server` beside it.
- **Next on `main`, in round 101:** ABI 29 (100000-KRN-0222). If your run starts after it lands, rebuild everything, as with each ABI.
