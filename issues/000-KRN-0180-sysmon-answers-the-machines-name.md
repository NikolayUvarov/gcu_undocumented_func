# 000-KRN-0180 — `sysmon` answers the machine's name (`sysinfo.wit` 4.2)

**Type:** `sysmon`, `idl/sysinfo.wit`, `init` (a read-only scope) · **Owner:** `KRN` · **Priority:** P1 (the maintainer, 2026-11-05) · **Status:** in progress · **Blocked by:** — · **Main task:** — · **Constitution:** MC-3.1, MC-10.2, MC-12.4

**Asked by:** EFF with LNK, `ALL/2026-10-28T0407Z-EFF-the-machines-name.md` on `internal-dev-messages`, item 3, after the maintainer's word of 2026-10-28: the machine's name, set on the machine itself.

## Problem

- **Who needs the name.** The Effector agent, `link` and `netstack` need the machine's name.
- **Why they cannot read it.** None of them holds a client of `data/`:
  - the agent's scopes are `system/effector` and `data/effector`;
  - `link`'s are `data/link` and `data/firmware`.
- **The proposed source.** One setting that APP keeps on the boot disk (proposed: `data/hostname`, one RFC 1123 label, `mindcore` when unset).

## Plan

1. **`sysinfo.wit` 4.2:** `hostname() -> string<63>`, answered by `sysmon` to every client.
   - It reads the file at each call, or when it changed, and checks it is a label.
   - `mindcore` is the answer when the file is missing or not a label, and then `sysmon` logs why.
2. **`sysmon` gets a read-only `vfs_server` scope of the name's directory alone.**
   - The KRN track asks APP to keep the file in a directory of its own, for example `data/machine/name`, so that this scope reaches nothing else (MC-3.1).
   - A scope of all of `data/` is not given.
3. **Clients** keep the `sysmon` client they already hold:
   - `effector_gw`;
   - `link` in developer mode;
   - the shell.
4. **`netstack` starts before `sysmon`,** so it gets the same read-only scope of the name's directory. Its use is the network track's [000-NET-0050](000-NET-0050-dhcp-sends-the-machines-name.md).

## Acceptance criteria

- **Host tests:** the label check (length, characters, hyphens), and the default.
- **QEMU, x86:**
  - with the file set to `dexp`, `sysinfo hostname` (or a shell command over the client) answers `dexp`;
  - changed while running, the next call answers the new name;
  - missing or invalid, it answers `mindcore` and logs why;
  - `stat caps` shows `sysmon`'s scope and nothing wider.
- **Docs:** `docs/api` and the profile's `sysmon` row.

## The file (2026-11-05)

APP chose it in [000-APP-0123](000-APP-0123-the-machines-name-and-hostname.md): `data/machine/name`, a directory of its own, as point 2 asks.
- **What it holds:** one RFC 1123 label, with an optional line end.
- **Capital letters** are taken as small ones when the name is set. A file that holds capitals was not written by the shell, so `sysmon` reads it as holding no name, as APP's `read` does.
- **A missing file, or one that is not a label,** gives `mindcore`.
- **Who writes it:** APP's `hostname <name>`. Until that command lands, the owner can write the file by hand.

## Progress

2026-11-05, on `claude/000-KRN-0180-hostname`:
- **`sysinfo.wit` 4.2:** `hostname() -> string<63>`. `libmind/src/idl/sysinfo.rs` is generated again.
- **`mind::hostname`** (`libmind/src/hostname.rs`) holds the directory, the file, the default and APP's rule (`label`, `read`). The shell may use it in place of its own copy.
- **`sysmon`** answers `hostname` from `name` in its `SLOT_VFS` scope, at each call. It logs `[SYSMON] THE MACHINE'S NAME: <name> (<source>)` when the name or its source changes, and answers `mindcore` when the file is missing, holds no label, or cannot be read.
- **`init`** gives `sysmon` a read-only scope of `data/machine/` (`scopes_of`), made if missing. A name set later is then read without a restart. A volume that cannot be written gives no scope, and `init` says so.
- **`bench`'s head:** `MIND Core on <name>, …` in `check`, `bench` and `kbench`, on the screen and in their logs.
- **Tests:**
  - host: `tests/hostname_host.rs`;
  - QEMU: `sysmon_name_check` in the `vfs` suite; `kbench_check` requires the head's name; `edit_suite`'s listing of `data/` now holds `machine`.
- **Docs:** `docs/api/README.md` (`hostname`), `docs/profile/bootstrap.md` (`sysmon`).

2026-11-05, after `main` brought APP's `hostname` (round 86):
- **The two checks had one name.** APP's `hostname_check` in the `shell` suite and this task's in the `vfs` suite were both `hostname_check`, and Python keeps the later. The merge raised no conflict, so APP's check would no longer have run. This task's is now `sysmon_name_check`.
- **A removed `data/machine/`.** APP's check ends with `rm data/machine`. `vfs_server` ends a scope of a removed directory, so `sysmon` then answers `mindcore` (`NOT READ`) until it starts again. At that start `init` makes the directory and a new scope. `sysmon_name_check` now checks both, and the profile's `sysmon` row says so.
- **A volume without `data/`.** The first x86 run (x32-128, dd7fc88b) failed in two suites, for one reason: `init` made `data/machine/` with its parents at each boot.
  - **`tools`:** `fm_check` (APP's) found `data/` in a root that had none, and `fm /` opened on it.
  - **`vfs`:** `ls data` counted one entry more.

  `init` now makes the directory only below a `data/` that is there (`Make::InParent`). A volume without `data/` gives `sysmon` no scope, and `init` logs `NO FILES IN data/machine`. Its name is then read from `sysmon`'s next start. The `vfs` and `edit` suites count `data/machine/` in `data/`.
- **The `vfs` suite's image has no `data/` at boot,** so the run again (16e43f5e) found no scope, and its first `write data/machine/name` answered `NOT FOUND`. `sysmon_name_check` now starts there: `init`'s `NO FILES IN data/machine`, `mindcore` not read, then `mkdir data/machine` and `svc restart sysmon`, after which the name is read. The `edit` suite's image has `data/`, and its listing holds `machine`. The `shell` suite with APP's `hostname_check`, and `kbench_check`, passed on x86 (`keys, shell, tools`) and aarch64 (`programs, shell and four CPUs`).
- **The `disk` suite's `disk_check`** (fm, issue 048, now APP's) went down from `..` onto `tree` in `data/`; `machine` now comes first. Its one positional step takes one key more, a change of its expectation only, told to APP. On dfb513f1 the `vfs`, `disks` and `edit` suites passed on x86 (x32-128).
- **One rule, two copies:** `shell/src/hostname.rs` (APP's) and `mind::hostname` say the same rule word for word. Whether the shell uses `mind::hostname` once this lands is APP's choice.

**Tested,** through Effector:
- **x32-128, x86:** `files and block writes` passed on 776e6b24 (828 s); `keys, shell, tools` on 16e43f5e (419 s).
- **acc-2, aarch64:** `files and the block store` passed on dfb513f1 (786 s); `programs, shell and four CPUs` on 16e43f5e (427 s).
- The commits after 16e43f5e change only the `vfs` suite's check and `disk_check`.
- **Host:** `tests/hostname_host.rs` (4), and `sysmon`, `idl`, `idl_fuzz`, `shell`, `bench`, `services` and `fs_routing`, all passed. `scripts/mind_idl.py` regenerates nothing new.

All four criteria are met on those runs. The task closes once it lands.

**Returned in round 87** (candidate 0e04398e), for two failures from `init` making `data/machine/` where `data/` is:
- **`x86: the window manager`:** wm's Settings browsed a background in `data/` and found `data:machine` first, not `data:sky.bmp`;
- **`aarch64: boot and fault containment`:** `fsck` found the boot volume's dirty bit after a reset during a slot's own loader. A boot that wrote nothing before now made the directory, and the reset left the volume unclean.

**The change after it:**
- `init` never makes the directory (`make` false, as for `nnd`): a boot writes nothing for it again.
- `scripts/make_usb_image.py` puts `data/machine/` on every image it writes, so a written disk gives `sysmon` its scope and a name `hostname` sets later is read without a restart.
- A disk without the directory, the tests' images among them, gives no scope until `sysmon`'s next start after it is made.
- The `Make` modes are gone. The `vfs`, `edit` and `disk` suites' expectations are `main`'s again: `disk_check`'s extra key, told to APP, is taken back. `sysmon_name_check` makes the directory, and removes it at its end.

## Related

[000-NET-0050](000-NET-0050-dhcp-sends-the-machines-name.md), 750-EFF-0018, 800-LNK-0028, [000-APP-0123](000-APP-0123-the-machines-name-and-hostname.md) (the setting and `hostname`).
