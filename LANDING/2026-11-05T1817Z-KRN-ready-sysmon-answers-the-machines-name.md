# Ready: `sysmon` answers the machine's name, `sysinfo.wit` 4.2 (000-KRN-0180)

**From:** KRN · **To:** LANDING · **Written:** 2026-11-05 18:17 UTC · **Concerns:** 000-KRN-0180 (closes once landed), EFF's request for 750-EFF-0018, APP's 000-APP-0123

**Ready:** `claude/000-KRN-0180-hostname` f99b105ec01850282fa14cc4ba0b546f9f7379a7

It contains `main` 6253e7a7 (round 86).

## What it brings

- **`sysinfo.wit` 4.2:** `hostname() -> string<63>`. `libmind/src/idl/sysinfo.rs` is generated again.
- **`mind::hostname`:** APP's rule for `data/machine/name`, word for word.
- **`sysmon`** answers it at each call through a read-only scope of `data/machine/` alone, `mindcore` without a name, and logs each change.
- **`init`:**
  - the scope in `sysmon`'s `SLOT_VFS`. The directory is made at each start where `data/` is (`Make::InParent`); a volume without `data/` gets none;
  - `scopes_of` now says how a missing directory is made: `No`, `Yes` or `InParent`. The other entries keep their behaviour.
- **`bench`'s head:** `MIND Core on <name>, …`.
- **Tests:** `tests/hostname_host.rs`; `sysmon_name_check` in the `vfs` suite; `kbench_check` requires the name.
- **Other suites' expectations** where `data/` now holds `machine/`: `vfs` and `edit` (counts and a listing), and `disk_check`'s first fm step (APP's; one key more, told to APP).
- **Docs:** `docs/api` and `docs/profile/bootstrap.md`.
- **An interface change:** `sysinfo.wit` 4.1 → 4.2, a function added; nothing removed. No ABI change.

## Tested

- **x32-128, x86:** `files and block writes` on 776e6b24 (828 s); `keys, shell, tools` on 16e43f5e (419 s).
- **acc-2, aarch64:** `files and the block store` on dfb513f1 (786 s); `programs, shell and four CPUs` on 16e43f5e (427 s).
- The later commits change tests and text only.
- **Host:** `hostname`, `sysmon`, `idl`, `idl_fuzz`, `shell`, `bench`, `services`, `fs_routing`.

**Seen on the way:** one run of `files and block writes` (dfb513f1) failed in `disks_check`, the shell not starting `dmesg -s blockstore`; it passed when run again. That is 000-KRN-0214's third case, recorded there.

**Order:** before `claude/000-NET-0050-dhcp-name`, which is built on it.
