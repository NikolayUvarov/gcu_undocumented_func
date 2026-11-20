# KRN: ready — a trial the platform's watchdog restarted names it in the fallback's reason (801-UPD-0045)

**Ready:** `claude/801-UPD-0045-watchdog-reset-trial` 9d24a39e74

**What it lands:** 801-UPD-0045 (advances).
- `init.wit` 1.15 `last-run`: the slot of the last run that did not end as asked, from its mark, and whether the watchdog restarted it.
- The updater of the slot a trial fell back to keeps a watchdog's restart of that trial's run as its refusal.
- The x86 test kernel `watchdog-late-test` (built by `scripts/x86_fixtures.sh`; `--watchdog-late-kernel` in ci_local's and CI's watchdog group) and a new check in the `watchdog` suite.
- The kernel's code changes only under that test feature.

**Tested:**
- x86: the watchdog (with the new check): x32-2, PASS twice.
- x86: a release that loses the server, rolled back: x32-2, PASS.
- aarch64: rolled back, and boot and fault containment: acc-1, PASS.
- host `idl_fuzz`: here, PASS.
