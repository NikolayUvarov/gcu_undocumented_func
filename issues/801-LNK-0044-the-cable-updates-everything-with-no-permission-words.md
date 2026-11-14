# 801-LNK-0044 — The cable updates everything with no permission words

**Type:** link (`link`, `tools/mindlink`, `effector485`, `scripts/link_update.py`) · **Owner:** `LNK` · **Priority:** P1 · **Status:** open, the `LNK` side done (2026-11-14) · **Blocked by:** `KRN` (`init` starts `link` with no `link.conf` and grants its clients each time) and `UPD` (the updater takes `data/link/rel/` whatever `update.txt` says), both asked on 2026-11-14 · **Main task:** [801](801-the-bench-stays-manageable.md) · **Constitution:** MC-9.3, MC-9.4, MC-12.3

## Problem

The maintainer (2026-11-14): «effector485 должен уметь обновлять весь код, любой абсолютно без всяких разрешений». Today the cable's reach depends on words on the disk:
- `link.conf` must exist, with `run` and `dev`, for `replace`, `reboot`, the shell's lines and keys;
- `update.txt` must name `data/link/rel` for a slot update;
- without `link.conf`, `link` does not start at all.

## Plan

1. **`link` starts whenever a supported converter is attached.** `link.conf` only sets its speed and address.
2. **Over the cable, without any word:**
   - a slot update of every component, from the loader to the applications: the updater takes `data/link/rel` as a source whatever `update.txt` says;
   - `replace` of a service;
   - `reboot`, the log, files under `data/link/`, keys and the shell's lines.
3. **Releases stay signed** (MC-9). An unsigned one is refused, as now.
4. **`docs/profile/threat-model.md` and `docs/link/`:** the cable is physical access to the machine.

## Acceptance criteria

- **QEMU:** a disk with no `link.conf` and no `update.txt` takes a slot update and a `reboot` over the link's serial line.
- **On the bench:** the same on one machine.

## Progress

### What stopped each operation (`main` f32627e8, 2026-11-14)

**`link` not started at all.**
- `init/src/main.rs:1879-1883` (`boot_one`) skips `usb_serial`, `uart_serial` and `link` with `[INIT] <name> NOT STARTED: NO LINK.CONF` when `self.link` is `None`.
- The bootloader sets it from `LINK.CONF` at its volume's root (`bootloader/src/main.rs:141-155`).
- A file of 512 bytes or more counts as none: `read_file` refuses one that fills its buffer (`bootloader/src/main.rs:426`).

**`link.conf` without `run`.** `link` refused every RUN: `link/src/main.rs:216` (`[LINK] RUN … REFUSED: link.conf HAS NO "run"`). It made no endpoint for the programs' output either (`link/src/main.rs:786`, `793`).

**`link.conf` without `dev`.** Two gates, one in `link` and one in `init`.
- **In `link`, each refused DENIED and logged:**
  - a shell line (`link/src/main.rs:284`);
  - `replace` (`330`);
  - `reboot` (`345`);
  - keys (`355`);
  - a built-in program by name (`667`, which then looked in `data/link/`);
  - the lending of the network, the Wi-Fi station and the system monitor to a program (`229`).
- **In `init`, the clients granted only with `dev` (`link_dev()`, `init/src/main.rs:1788`):**
  - in the `"link"` arm (`init/src/main.rs:821-835`): the system monitor, the launch-only policy client, the station's, the lifecycle client for `reboot` and `replace`, the shell's and the input privilege;
  - in `spawn` (`init/src/main.rs:1419-1424`): the scope of `data/firmware/`;
  - in `read_modes` (`init/src/main.rs:1981`): `link-console` from `data/mode.conf` counts only with `dev`.
- Without those clients `link` answered UNSUPPORTED (`link/src/main.rs:285`, `331`, `346`, `432`).

**`link.conf` with a rate the line refuses.** `link` stopped serving (`link/src/main.rs:814`).
- Without `baud=` it asked 115200 (`link/src/main.rs:24`, `768`), while the bench's daemon runs at 460800.

**`update.txt` absent, or naming another source.**
- The updater's one source is `update.txt`'s `source` line (`updater/src/main.rs:131`).
- Without the file it logs `[UPDATER] NO update.txt: IT ACTS ONLY WHEN ASKED, AND HAS NO SOURCE` and keeps no source and `automatic none` (`updater/src/main.rs:108`).
- So `automatic` runs nothing (`updater/src/main.rs:1218`), and `check` and `fetch` answer `NoSource` (`611`).
- A release put into `data/link/rel/` is taken only with `source data/link/rel` and `automatic apply`. The shell cannot start it from the cable either: `update` is neither in `NOW` nor in `ASKED` of `shell/src/clients.rs`, so a link line `update apply` is refused (`clients.rs:66`).

### The `LNK` side (2026-11-14, branch `claude/801-LNK-0044-cable-without-permissions`)

- **`link` asks no word of `link.conf`.**
  - **What the file still sets** (`link/src/conf.rs`, host-tested in `tests/link_conf_host.rs`): only `baud=` and `address=`.
  - **The rate without `baud=`:** 460800 bit/s, the bench's, then 115200 where the line refuses that. A named rate the line refuses falls back the same way.
  - **Every operation is decided by `init`'s grant for it.** A missing client is refused DENIED and logged: `INIT GAVE LINK NO SHELL CLIENT`, `NO LIFECYCLE CLIENT`, `NO INPUT CAPABILITY`.
  - **Programs** start from `data/link/` and by built-in name with no word. The endpoint for their output is always made.
  - **The log** says once what `link` holds: `[LINK] NO WORD IN link.conf LIMITS THE CABLE (801-LNK-0044); FROM INIT: …`.
  - **The mark** is red while `init`'s grants let the master change the machine beyond `data/link/`.
- **`mindlink` and `effector485`:** their help and refusal texts name the client `init` must grant, not a word in `link.conf`.
- **`scripts/link_update.py`:**
  - it asks nothing of `update.txt`, and its docstring says what the machine needs today and after the requests;
  - `--channel` names the channel to publish;
  - an updater that says it has no source and names no `data/link/rel` is refused before anything is put. Host-tested in `tests/link_update_test.py`.
- **`docs/link/` (English and Russian)** and **`docs/profile/threat-model.md`:** the cable is physical access, what it reaches through which grant, and that releases stay signed.
- **The acceptance check:** `cable_without_words_check` in the `linkupdate` suite. With no `link.conf` and no `update.txt` it runs a `reboot` over the link, then a slot update. Each step prints a SKIP naming the request it waits for, until that request lands.

### Evidence (2026-11-14, local, x86)

- **Host:** `scripts/host_tests.sh` passed whole. That includes `tests/link_conf_host.rs`, `tests/link_update_test.py`, and `tools/mindlink`'s tests over a pty and TCP.
- **QEMU, 4 CPUs:**
  - **the `link` suite passed.** Its first disk's `link.conf` holds no `run`. The second holds only `address=5`: link served there at 460800 bit/s and started programs. It refused a shell line, `reboot`, `replace` and keys, each logged with the client `init` did not grant;
  - **the `linkupdate` suite passed** with an empty `link.conf`. `cable_without_words_check` then printed `SKIP: … init starts link only when the boot volume holds link.conf`.
- **Against stand-ins, never committed:** `init` patched to start `link` with no `link.conf` and to grant it the developer mode's clients each time, and the updater patched to take `data/link/rel` with no `update.txt`. There `cable_without_words_check` passed on a disk with neither file:
  - link served at 460800 bit/s;
  - `reboot` over the link restarted the machine;
  - release 6 put into `data/link/rel/` was taken into slot B, booted on trial and confirmed.

  This is evidence for the `LNK` side and the check in QEMU on x86 alone (MC-12.1, 12.2), not for the requests' own code.
- **aarch64:** `link` builds; its suites have not run.

### What remains

The two requests below, sent on 2026-11-14 by message. Then the QEMU criterion is `cable_without_words_check`'s PASS line, and the bench's is one machine.
- **`KRN` (`init`):**
  - start `usb_serial` and `link` with no `link.conf`;
  - grant `link` the developer mode's clients every time it starts, but the input privilege in a safe start;
  - count `link-console` without `dev`.
- **`UPD` (the updater):**
  - `data/link/rel/` a source always, channel `stable`, applied by itself, with every signature, digest and floor check as now (MC-9.2–9.4);
  - a log line that names the directory.

## Related

[801](801-the-bench-stays-manageable.md), [800](800-mind-link.md), [351-LNK-0013](351-LNK-0013-staging-a-release-through-the-link.md).
