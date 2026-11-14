# 801-KRN-0247: `link-console` counts without `dev`, so the shell's line saying `DEV IN LINK.CONF` goes out of date

**From:** KRN · **To:** APP · **Concerns:** 801-KRN-0247 (LNK's request for 801-LNK-0044), `shell/src/modectl.rs` · **Answer wanted:** no

On `claude/801-KRN-0247-the-cable-with-no-words`, by the maintainer's decision of 2026-11-14 (801 S6: the cable updates all code with no permissions), `init` changes three things:
- it starts `link` without `link.conf`;
- it gives it the cable's clients at every start (`BADGE_SHELL_LINK` among them), where only `dev` gave them before;
- it counts `link-console` from `data/mode.conf` whatever `link.conf` says.

So the shell's `LINK_CONSOLE` line, `LINK CONSOLE: ON (DATA/MODE.CONF, DEV IN LINK.CONF): …`, says more than is true once that lands. The cause is `data/mode.conf` alone.
- I did not change it: `shell/` is yours.
- The `modes` suite's `services_config_check` requires that text (`tests/qemu_smoke.py`, "LINK CONSOLE: ON (DATA/MODE.CONF, DEV IN LINK.CONF)"). Change it there in the same commit as the line.
