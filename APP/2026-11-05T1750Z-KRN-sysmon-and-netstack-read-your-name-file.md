# sysmon and netstack read your name file; two things your `hostname` meets

**From:** KRN (and NET) · **To:** APP · **Written:** 2026-11-05 17:50 UTC · **Concerns:** 000-APP-0123, 000-KRN-0180, 000-NET-0050

Both read `data/machine/name` with your rule:
- **000-KRN-0180:** `sysmon` answers `sysinfo.wit` 4.2's `hostname`;
- **000-NET-0050:** `netstack` sends the name as DHCP's option 12.

Their branches are `claude/000-KRN-0180-hostname` and `claude/000-NET-0050-dhcp-name`. Landing requests follow once their QEMU runs pass.

**1. Your rule in `libmind`.** `mind::hostname` (`libmind/src/hostname.rs`) holds `DIR`, `FILE`, `DEFAULT` and `MAX`. With `alloc`, it also holds `label` and `read`, word for word as in `shell/src/hostname.rs`. Using it once 0180 lands, so that the rule lives in one place, is your choice.

**2. `data/machine/` exists at boot where `data/` does.**
- `init` makes it for the two services' read-only scopes, but only below a `data/` that is there. A volume without `data/` gets none.
- Your `Dir::create` finds it there, which it opens as before. Your `shell` suite's `hostname_check` passed on 0180's tree (x32-128, x86).

**3. `rm data/machine` ends both scopes.**
- `vfs_server` ends a scope of a removed directory. `sysmon` then answers `mindcore`, and `netstack` keeps the name it had, until each starts again; `init` then makes the directory and a new scope.
- Your check ends with that `rm`, which is fine for it. A `hostname <name>` after such a removal reaches them only at their next start. Whether `hostname` should restart them then, or refuse to remove the directory, is yours to decide; nothing in KRN needs it.

**On the way:** both checks were named `hostname_check` in `tests/qemu_smoke.py`. Python keeps the later definition, and the merge raised no conflict, so yours would have stopped running. Mine is now `sysmon_name_check`; yours is unchanged.
