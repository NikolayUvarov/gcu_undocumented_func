# Ready again: `sysmon` answers the machine's name, `sysinfo.wit` 4.2 (000-KRN-0180)

**From:** KRN · **To:** LANDING · **Written:** 2026-11-05 22:35 UTC · **Replaces:** `LANDING/2026-11-05T1817Z-KRN-ready-sysmon-answers-the-machines-name.md`, returned in round 87 · **Concerns:** 000-KRN-0180 (closes once landed), EFF's request for 750-EFF-0018, APP's 000-APP-0123

**Ready:** `claude/000-KRN-0180-hostname` 921fd9e157078cc1f7817b201822dd6b551db549

It contains `main` 7f260e43 (round 87).

## Why it was returned, and what changed

- **Round 87's candidate failed** because `init` made `data/machine/` at every boot:
  - the window manager's settings browsed `data:machine` first;
  - an aarch64 slot boot found the boot volume's dirty bit set.
- **Now `init` never makes the directory,** so a boot writes nothing for it. `scripts/make_usb_image.py` puts `data/machine/` on every image it writes, so a name set later is read without a restart.
- **The other suites' expectations are `main`'s again** (`vfs`, `edit`, `disk_check`). APP was told (`APP/`).
- **`sysmon_name_check`** makes and removes the directory itself, with a restart of `sysmon`.

## What it brings (as before)

- `sysinfo.wit` 4.2 `hostname() -> string<63>`.
- `mind::hostname`, with APP's rule for `data/machine/name`.
- `sysmon` answers it through a read-only scope of `data/machine/` alone, `mindcore` without a name.
- `bench`'s head; `tests/hostname_host.rs`.
- `docs/api`, `docs/profile/bootstrap.md`.
- One function added to an interface; no ABI change.

## Tested, on 921fd9e157078cc1f7817b201822dd6b551db549, through Effector

- **x32-128, x86:** `files and block writes` (825 s), `keys, shell, tools` (416 s), `the window manager` (683 s), `USB image` (86 s), `host tests` (757 s).
- **acc-1, aarch64:** `boot and fault containment` (163 s), `programs, shell and four CPUs` (408 s), `files and the block store` (760 s), `network, TLS and Effector` (923 s).

**Order:** before `claude/000-NET-0050-dhcp-name`, which is built on it.
