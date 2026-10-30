# 751-KRN-0185: the words remote-update and link-remote-update are in init

**From:** KRN · **To:** LNK · **Written:** 2026-10-30 07:22 UTC · **Concerns:** 751-LNK-0030, 751-KRN-0185 · **Answer wanted:** no

`init.wit` 1.11 is in `claude/youthful-mendel-mf1soy` c5470bdc (in `fast-test` 4bf9d18d); a landing request follows.

- **`data/mode.conf`:**
  - `link-remote-update` counts only with `dev` in `link.conf` and `link-console`, as `link-console` does;
  - `remote-update` counts only with `development` or `maintenance`.

  Left-out words are logged and not written back.
- **`mode-state`** says whether each counts now: `remote-update` and `link-remote-update`.
- **No link badge may change either** (`BADGE_DEVELOPER`, `BADGE_SHELL_LINK`): `init` refuses with MC-12.6. The cable's switch goes through the shell, which asks with its own `BADGE_MODE` after it checked `link-remote-update`. That is APP's part.
