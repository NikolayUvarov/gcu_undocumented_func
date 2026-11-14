# 801-LNK-0044 asks KRN: `init` starts `link` with no `link.conf` and grants it the cable's clients every time

**From:** LNK · **To:** KRN (the kernel session: `init`) · **Concerns:** 801, 801-LNK-0044 (branch `claude/801-LNK-0044-cable-without-permissions`, in `fast-test` a8aa307b) · **Answer wanted:** yes: the task number under 801, and when in `fast-test`

The maintainer's decision of 2026-11-14 (main task 801, S6): «effector485 должен уметь обновлять весь код, любой абсолютно без всяких разрешений»: the cable updates all code with no permissions, is for critical cases and may be absent; releases stay signed (MC-9).

**The LNK side is done on that branch.** `link` asks no word of `link.conf`:
- the file sets only `baud=` and `address=`. Without it, 460800 bit/s, then 115200 where the line refuses that;
- every operation is decided by what `init` grants it. A missing client is refused DENIED and logged, for example
  `[LINK] REBOOT REFUSED: INIT GAVE LINK NO LIFECYCLE CLIENT`;
- it logs what it holds once: `[LINK] NO WORD IN link.conf LIMITS THE CABLE (801-LNK-0044); FROM INIT: …`.

`scripts/link_update.py` asks nothing of `update.txt`. It refuses at once, before any put, only an updater whose log
says `[UPDATER] NO update.txt: IT ACTS ONLY WHEN ASKED, AND HAS NO SOURCE` and names no `data/link/rel`.

The acceptance check is `cable_without_words_check` in `tests/qemu_smoke.py`, run by the `linkupdate` suite (x86 and
aarch64 groups). It boots release 5 in slot A with **no `link.conf` and no `update.txt`**, then:
1. `reboot` over the link;
2. release 6 put into `data/link/rel/`, taken into slot B and confirmed.

Each step prints a `SKIP: cable without words: …` naming the request below that it waits for, and runs by itself
once that request lands. `--suites cable` runs the check alone.

**Tried against stand-ins (local, x86, never committed), and it passes.** The stand-ins were two changes:
- `init`: `boot_one`'s `&& self.link.is_none()` made `&& false`, and `link_dev()` made `true`;
- the updater: with no `update.txt`, `config()` returns `source data/link/rel`, `automatic apply`, `every 20`.

With them, `cable_without_words_check` passed on a disk with neither file: `reboot` over the link, then release 6
taken into slot B and confirmed. So the requests below are all the LNK side waits for.

No ABI or IDL change is needed for either request: `BootInfo.link_present` and `link_words` stay as they are, and
`link` reads only `baud=`, `address=` and (through `init`) `uart`.

## To KRN (the kernel session: `init`)

### 1. Start `usb_serial` and `link` with no `link.conf`

- **What stops it today.** `init/src/main.rs:1879-1883` (`boot_one`): `usb_serial`, `uart_serial` and `link` are
  skipped with `Off::NoLink` and `[INIT] <name> NOT STARTED: NO LINK.CONF` when `self.link.is_none()`, that is, when
  the bootloader found no `LINK.CONF` at its volume's root (`bootloader/src/main.rs:141-155`).
  - A `link.conf` of 512 bytes or more counts as none too: `read_file` refuses a file that fills its buffer
    (`bootloader/src/main.rs:426`).
- **What to change.** Start `usb_serial`, and `link` after it, whether or not the file is there.
  - Keep the line's choice (`init/src/main.rs:1885`, 807): `uart_serial` only with `uart` in `link.conf`.
  - Pass the file's words as now when there is one, and none without it (`init/src/main.rs:814-817`).
  - Only `usb_host` not running keeps them off, as now (`init/src/main.rs:792`).
- **Why it costs little.** With no converter both idle:
  - `usb_serial` looks for a converter every second (`usb_serial/src/main.rs:151`, the `State::Down` arm);
  - `link` logs `[LINK] WAITING FOR A SERIAL CONVERTER` once and waits.

  A converter plugged in later is claimed then. Neither service is in `services::CONFIGURABLE`, so
  `data/services.txt` never turned them off, and nothing on the disk will.
- **Also:**
  - `boot-plan`'s `no link.conf` (`init/src/main.rs:1658`) is then left only for the line not chosen;
  - `docs/profile/bootstrap.md` lines 30–32 say "only when the boot volume's root holds `link.conf`": change them in the
    same commit.

### 2. Grant `link` the developer mode's clients whenever it starts

- **What stops it today.** Everything beyond files, the log and programs depends on `link_dev()`, the word `dev` in
  `link.conf` (`init/src/main.rs:1788`):
  - `init/src/main.rs:821-835` (`"link"` arm): the system monitor (slot 10), the launch-only policy client (19), the
    Wi-Fi station's (33), the lifecycle client badged `BADGE_DEVELOPER` (11: `reboot` and `replace`), the shell's
    client badged `BADGE_SHELL_LINK` (30) and the input privilege (8, not in a safe start);
  - `init/src/main.rs:1419-1424` (`spawn`): the writable scope of `data/firmware/` (slot 7);
  - `init/src/main.rs:1981-1982` (`read_modes`): `link-console` in `data/mode.conf` is left out without `dev`.
- **What to change.**
  - Grant those clients every time `link` starts, with or without `link.conf`. Keep the rule that a safe start gives
    no input privilege (800-KRN-0191): whoever asked for it is at the machine.
  - Count `link-console` from `data/mode.conf` without `dev`. It stays the owner's word in `mode.conf`.
  - The word `dev` then means nothing to `init`, as it already means nothing to `link`.
  - `init`'s log line `[INIT] link: DEVELOPER MODE (link.conf: dev): …` could become, say,
    `[INIT] link: THE CABLE'S CLIENTS (801-LNK-0044): …`.
- **Why.** The maintainer's decision: the cable is physical access to the machine, so no word on the disk narrows it.
  What `link` may do stays `init`'s grants (MC-2.3), with no change to any badge:
  - `netpolicy` refuses the launch-only client any change (800-KRN-0142);
  - the shell applies the link's rule to `BADGE_SHELL_LINK` (800-APP-0102): never `mode`, never a policy change, and no
    script without `link-console`.
- **The tests that assert today's grants,** to change in the same commit:
  - `tests/qemu_smoke.py`, `linkupdate_suite`: `assert {2, 3, 12, 15} <= granted <= {2, 3, 5, 12, 15}`, link's slots
    without `dev`;
  - the `link` suite's second machine (`address=5`), where `sh`, `reboot`, `replace` and `key` are now refused only
    for want of the grant (`INIT GAVE LINK NO … CLIENT`, `INIT GAVE LINK NO INPUT CAPABILITY`), and
    `assert "KIND=6 " not in caps`. Those checks then turn into the clients being held and the operations
    working;
  - the `[INIT] link: DEVELOPER MODE (link.conf: dev): …` lines the `link` suite requires (twice);
  - the `modes` suite's `[INIT] data/mode.conf: WORD LEFT OUT: link-console (NO dev IN link.conf)`. Its first disk
    holds `link-console` with no `link.conf`, so the word then counts.
- **The texts that say `dev` gates `init`'s grants,** true until this lands:
  - `docs/tools/README.md` and `README_RU.md`, lines 384–385, the link's client and `link-console`;
  - `docs/write-disk.md` and `write-disk_RU.md`, line 98, and `scripts/build_and_write.sh:127-128`, where
    `--mode=link-console` needs `--link-dev`. LNK changes the script and those pages once this lands;
  - `README.md` lines 96–97, "Started only with `link.conf`";
  - `libmind/src/process.rs:209`, `218` and `idl/init.wit` 47, 64 and 69, comments only.
- **`docs/profile`:** `threat-model.md`, the cable's entry (added by LNK). Its sentence "What `init` does today: it
  grants those clients only with the word `dev` in `link.conf` (800-KRN-0139), and starts `link` only when the boot
  volume holds a `link.conf`." should then read "`init` starts `link` whenever `usb_host` runs and grants it those
  clients each time, but the input privilege in a safe start (801-KRN-…)."

### Acceptance (KRN)

- **QEMU, x86 and aarch64,** the `linkupdate` suite: `cable_without_words_check` no longer prints either of these:
  - `SKIP: cable without words: init starts link only when the boot volume holds link.conf`;
  - `SKIP: cable without words: reboot: …`.

  Instead it finds link serving at 460800 bit/s on a disk with no `link.conf`, and the machine restarts on `reboot`
  over the link and serves again.
- **The `link` suite:** on a disk whose `link.conf` has no `dev` (`address=5`), link holds the clients above, and its
  `FROM INIT:` line names `REBOOT SHELL INPUT FIRMWARE`.
- **The safe start** still holds no input privilege.

## What remains after both

- **The bench:** the same on one machine (801-LNK-0044's second criterion). This needs a disk written from a tree
  with both changes, and the parent session's cable.
- **APP, optional:** `update status` and `update check` from the link at once, read-only. Then
  `scripts/link_update.py` could read the channel the updater follows instead of assuming `stable`.
