# Ready: 801-KRN-0247 and 801-UPD-0039, the cable with no words on the disk (801 S6), with 801-LNK-0044's halves

**Ready:** `claude/801-KRN-0247-the-cable-with-no-words` ec0264e037

It contains `main` 32272bba66, LNK's `claude/801-LNK-0044-cable-without-permissions` (0568ece2e5) and UPD's `claude/801-UPD-0038-an-update-from-the-previous-release` (015323ef5c, requested at 1403Z). It is in `fast-test` (8c067849f7).

- **Advances** 801 S6:
  - 801-KRN-0247: `init` starts `usb_serial` and `link` with or without `link.conf`, gives `link` the cable's clients at every start with or without `dev`, and counts `link-console` in `data/mode.conf` without `dev`;
  - 801-UPD-0039: the updater always reads the cable's directory `data/link/rel/`, on its own timer, whatever `mode.conf` says;
  - 801-LNK-0044's `init` and updater halves, with LNK's commit: `link_update.py` needs no `update.txt`.

  The bench part (the three machines) stays open in each issue.
- **Lands with** APP's 801-APP-0141 (1357Z), whose texts are true before and after.
- **Merges** onto `main` with the queue's four requests without a conflict in code; only `issues/README.md` conflicts.
- **Tested** on `ec0264e037`, through Effector:
  - acc-1: build (aarch64); `aarch64: the link` (222 s); `aarch64: an update through the link` (3095 s);
  - x32: build (x86); `x86: the updater and an update through the link` (2030 s); `x86: services, storage, audio` (800 s);
  - i9: `x86: an update from the previous release`, `main` → this branch (129 s).
- **Built** here, x86: `main` with the queue's four requests and this branch merged (991 s).
