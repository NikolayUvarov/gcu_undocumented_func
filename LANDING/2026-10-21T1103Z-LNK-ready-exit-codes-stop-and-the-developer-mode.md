# Ready: the link's exit codes, STOP and developer mode

**From:** LNK · **To:** LANDING · **Written:** 2026-10-21 11:03 UTC · **Concerns:** 800-LNK-0009, 0010, 0011, 0012; 351-LNK-0013 (numbered here)

**Ready:** `claude/LNK-link` 2fe75f9f009906e5f807986dbcf334c383709166

## What it brings

- **`link` with 800-KRN-0137 and 0138:** a program's exit code and STOP through the loader's `stop` and `watch`.
- **800-LNK-0012, the developer mode** (`dev` in `link.conf`):
  - built-in programs by name;
  - the four grants of 800-KRN-0139 lent to the programs `link` starts, with a flow bound by the policy broker for `REQUEST_NETWORK`;
  - REBOOT in `linkproto`, `link`, `mindlink reboot` and `effector485`'s `reboot`.
- **The `link` suite:**
  - exit codes, a stop;
  - the developer mode's part, with a VirtIO network card and `netpolicy.txt`;
  - its check of `link`'s grants allows the developer mode's four and `link`'s own endpoints.
- **Text:**
  - the MacBook Pro's runs in 800-LNK-0009, 0010 and 0011;
  - KRN's request for 351-KRN-0141 numbered as 351-LNK-0013, and `issues/requests-LNK.md` removed (351-KRN-0141's two links point to the task now).

## `main` in it

98586f1. It also merges cleanly with `claude/550-DRV-0036-radio-counts`, whose request is posted beside this one.

## What was run

On the link track's WSL machine, x86, 4 CPUs:
- `scripts/mind_idl.py --check`: up to date;
- the host tests of `linkproto` (17), `mindlink` and `effector485`: passed;
- **the QEMU `link` suite on x86: PASS, with no part skipped:**
  - `link` held the developer mode's grants 10, 11, 19 and 33;
  - `memtest` ran by name;
  - `netcheck ping:10.0.2.2` was answered;
  - `wifi status` and `uptime` ran;
  - REBOOT restarted the machine.

On the MacBook Pro (`fast-test` d2daa430), over RS-485:
- exit codes 3 and 0;
- `mindlink stop`;
- `effector485 cancel`.
