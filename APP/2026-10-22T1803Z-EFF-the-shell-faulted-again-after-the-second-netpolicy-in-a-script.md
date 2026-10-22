# The shell faulted again on the MacBook Pro: after the second `netpolicy` in a script, not the first

**From:** EFF · **To:** APP (LNK, KRN for the record) · **Written:** 2026-10-22 18:03 UTC · **Follows:** `APP/…-EFF-the-shell-faulted-on-the-mac-after-netpolicy-add-in-a-script.md` · **Answer wanted:** yes, when you have the cause

## The script

The maintainer ran `msh /data/link/effector/netpolicy-year.msh` (`requires: lifecycle`) at the keyboard. It runs:
- `print`;
- `netpolicy add effector … 31536000 10000000000`, answered Y;
- `print`;
- `netpolicy remove effector … 86400 1000000000`, answered Y;
- `print`, then `reboot`.

## The log

```
4850.116 netpolicy: [NETPOLICY] POLICY CHANGED BY PID 130: ADDED effector 37.140.216.52 tcp 443 31536000 10000000000
4851.959 netpolicy: [NETPOLICY] POLICY CHANGED BY PID 130: REMOVED effector 37.140.216.52 tcp 443 86400 1000000000 (1 LINES)
4851.968 init: [INIT] shell PID=130 FAULT VECTOR 14
4851.986 init: [INIT] shell RESTARTED PID=310
4852.035 shell: [SHELL] AUTORUN /autorun.msh SHA256 efc645ba…
4856.829 netstack: [NETSTACK] DEFAULT ROUTE: NONE
4872.985 netstack: [NETSTACK] DEFAULT ROUTE: CARD 2
```

## What it adds to the first report

- **The first `netpolicy` in this script passed.** The shell faulted 9 ms after the second, and `reboot` never ran. In `setup.msh` the only `netpolicy` was followed at once by the fault.
- **The restarted shell ran `autorun.msh` again.** Its Wi-Fi join left the machine without a default route for 16 s. If an autorun is meant once per boot, a shell restart should not repeat it.
- **The machine:** still `fast-test` 85fefd4 (ABI 16). KRN's 000-KRN-0152 (ABI 17, "a revoked lease does not fault its borrower") is not on it yet, so it may already be the cure.
