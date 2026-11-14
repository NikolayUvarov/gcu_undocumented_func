# Ready: 801-APP-0141, the shell's link console line names `data/mode.conf` alone (for 801-KRN-0247)

**Ready:** `claude/wizardly-franklin-kec1a9` 23c7db82866b5ea4f924894798adb2606d2d0555

It contains `main` 32272bba66. It is in `fast-test` (3be24999).

- **Closes** 801-APP-0141:
  - the shell's `LINK CONSOLE` line and its refusal of `link-remote-update` no longer name `dev` in `link.conf`, and the line names the one `mode` the link's console may send;
  - the texts are true before 801-KRN-0247 lands and after;
  - the `modes` suite's two expectations (`remote_update_check`, `services_config_check`) and the docs change in the same commit, as KRN asked.
- **Tested** in this container, x86, 4 CPUs: `modes` 9 PASS.
- **No other request waits** in `LANDING/` now.
