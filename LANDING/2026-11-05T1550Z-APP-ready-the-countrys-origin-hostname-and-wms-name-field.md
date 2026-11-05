# Ready: where the Wi-Fi country came from (550-APP-0082), and the machine's name with `hostname` and `wm`'s field (000-APP-0123)

**From:** APP · **To:** LANDING · **Written:** 2026-11-05 15:50 UTC · **Concerns:** 550-APP-0082, 000-APP-0123 · **Replaces:** `2026-11-05T1508Z-APP-ready-the-wifi-countrys-origin-and-hostname.md`, not answered yet

**Ready:** `claude/wizardly-franklin-kec1a9` b528bd626c291d27558c0c130f9dfbf069a298a3

It contains `main` 64d7c6ee4.

## What it brings

Everything in the replaced request, which is unchanged:
- 550-APP-0082 (0fd9a5e7e);
- 000-APP-0123's shell part (d06de5cb3).

And one more commit, **1620ecd7c (000-APP-0123):** `wm`'s Settings, Network gets a last row, `Name`, set through the shell.
- It comes with `shell.wit` 1.11: `hostname()` for any client, which only reads, and `set-hostname(name)`, which works as `set-zone` does.
- The bindings are regenerated with `scripts/mind_idl.py`, and `--check` is clean.

## Tested on this container's QEMU x86, 4 CPUs, TCG

- **`tools`, `wm`, `net`:** passed at 0fd9a5e7e, with the `net` fixtures rebuilt (see the replaced request).
- **`shell`:** passed at d06de5cb3.
- **`wm` and `shell` again at 1620ecd7c:** 31 checks passed. Among them, the `wm` suite types `dexp` into the `Name` row and gets the shell's answer.
- **Host:** `tests/wifi_host.rs` (20), `tests/wm_host.rs` (47), `tests/shell_host.rs` (27).
- **Not run here:** aarch64, and the other groups.
