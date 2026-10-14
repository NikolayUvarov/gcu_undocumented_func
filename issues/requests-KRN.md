# Requests for the kernel track (KRN), not numbered yet

**Owner:** kernel track · **Status:** open (13 requests waiting, 2026-10-14, three of them for 750, one for 550-APP-0033, one for 000-NET-0015 and 000-APP-0049, one for 550-NET-0018 and 550-NET-0024, one for 351-NET-0022; a drive that comes back checked to hold the same volumes became [000-KRN-0079](../issues-done/000-KRN-0079-a-returning-drive-is-checked-to-be-the-same-medium.done), the BCM4331's core stopped before its bus mastering became [550-KRN-0082](../issues-done/550-KRN-0082-the-bcm4331-core-stopped-before-its-bus-mastering.done), the badge right became [000-KRN-0073](../issues-done/000-KRN-0073-a-badge-set-only-with-the-right-to-set-it.done), the IDL fuzzer in CI became [000-KRN-0069](../issues-done/000-KRN-0069-the-idl-fuzzer-in-ci.done), bus mastering off at boot [550-KRN-0070](../issues-done/550-KRN-0070-bus-mastering-off-until-a-driver-has-the-device.done), blockstore's quota [251-KRN-0071](../issues-done/251-KRN-0071-a-memory-quota-for-blockstore.done), the FAT free count [251-KRN-0072](../issues-done/251-KRN-0072-free-clusters-counted-a-fat-sector-at-a-time.done); QEMU's vvfat crash in the aarch64 boot suite is [000-KRN-0067](../issues-done/000-KRN-0067-the-aarch64-boot-test-on-pcu.done))

The kernel track numbers its own tasks (`NNN-KRN-MMMM`), so requests from other tracks wait here. The kernel track turns each into a task and removes it from this file. The file is kept while empty because other issues link to it; a new request goes below this line.

The request on `bcm_wifi`'s DMA region and interrupt became [550-KRN-0091](550-KRN-0091-bcm-wifi-dma-and-its-interrupt.md) (2026-10-12).

## A client of the updater for the shell

**Recorded by:** the update track (`UPD`), 2026-10-10, for [351-UPD-0007](../issues-done/351-UPD-0007-updater-service.done) and the tools track's `update` command (351-APP-0029, on its branch).

### Problem

`updater` serves `idl/update.wit` 1.0 (`check`, `fetch`, `apply`, `rollback`, `status`), but no task holds a client of it: it acts only through the automatic policy in `update.txt`. The shell's static slots 1 to 31 are all named in `common/abi.rs` (`SLOT_DYNAMIC` is 32), so a slot for the client is an ABI question.

### Plan (a proposal; the kernel track decides)

- A slot for the shell's client of `updater`, or another way for the shell to hold one.
- `init` gives the shell an unbadged client (the updater checks no badge: every request ends in a check of signed releases, and `apply` and `rollback` ask the user through the shell's command).
- The shell lends it to programs that ask for it, if the tools track wants `update` in `msh`.

### Acceptance criteria

The shell holds a client of `updater` (`caps` shows it), and `status` answers through it.

## init says whether it confirmed a trial boot

**Recorded by:** the update track (`UPD`), 2026-10-10, for [351-UPD-0007](../issues-done/351-UPD-0007-updater-service.done).

### Problem

`init` confirms a healthy trial boot to the kernel (`BOOT_CONFIRM`, 351-KRN-0014), and the confirmed boot record is the updater's to write. The updater cannot ask whether the boot was confirmed. It waits until 5 s past the kernel's deadline (120 s): had the boot not been confirmed, the kernel would have restarted the machine by then. That is sound but slow. The other slot stays untouched for two minutes after every update, and the QEMU `update` check waits that long.

### Plan (a proposal; the kernel track decides)

- `init.wit` 1.4: `boot: func() -> result<boot, error>`, with the slot, whether it booted on trial and whether `init` confirmed it. Any client of `init` may call it, or only the `BADGE_REBOOT` one.
- The updater asks for it every second while it waits, and keeps the deadline as the bound.

### Acceptance criteria

On a trial boot, the updater writes the confirmed record within a few seconds of `[INIT] TRIAL BOOT CONFIRMED`, and the `update` check passes.

## A launch session holds as many grants as there are launch slots (211-APP-0044)

**Recorded by:** the tools track (APP), 2026-10-10, for [211-APP-0044](../issues-done/211-APP-0044-console-joined-to-the-shell.done).

### Problem

`loader`'s launch session keeps at most 5 grants (`Session.grants: [(u8, usize); 5]` in `loader/src/main.rs`). A sixth `grant` is refused with `limit`. `LAUNCH_SLOTS` (211-KRN-0058) names 16 slots a launcher may fill.

`console` started by `wm` now asks for the shell's commands too, so it may need six grants:

- the window;
- the user's files;
- system information;
- the camera;
- the shell's commands (`SLOT_SHELL`);
- for `record -w`, the window to see.

The `wm` suite's `record -w` lost its window lease to the limit. The tools track now grants `SLOT_SHELL` last and lets a start go on without it (wm and the shell), so the program runs without the shell's commands when the session is full.

On the MacBook Pro `wm` itself now asks for six: the window manager's client, the user's files, system information, the camera, the shell's commands and the battery (211-APP-0070). The battery comes last and `wm` asks the shell for a copy when it did not fit (`shell.wit` 1.4 `battery`), so nothing waits on this; a larger session would make that call unneeded.

### Plan (a proposal; the kernel track decides)

- **The session's array** holds `LAUNCH_SLOTS.len()` grants, or `SPAWN_GRANTS_MAX` less the loader's own.
- **A refused grant** still answers `limit`.

### Acceptance criteria

A launcher grants each of `LAUNCH_SLOTS` in one session and the program holds them all. `console` from `wm` gets the shell's commands beside the window lease of `record -w`.

## A console program ends when its console is gone (000-APP-0056)

**Recorded by:** the tools track (`APP`), 2026-10-10, from the walk through wm's menu in [000-APP-0056](../issues-done/000-APP-0056-every-entry-of-wms-menu-reacts.done).

### Problem

wm's menu has a category "Tests and performance": `check`, `bench` and `kbench` run there in a `console` window. Closing that window ends `console`, but not the program it ran:

- `mind::output::send` stops sending when the launcher's endpoint answers `Peer`, and the program goes on.
- A full `kbench` or `bench` then measures for minutes with nobody watching it, and takes processor time from whatever the user does next.
- `console` cannot end its programs. Ending another program is the shell's (process control), and the shell asks the user first.

000-APP-0056 asks that nothing started from the menu runs unseen. Until this is settled, the wm suite waits for these three to end before it closes their window (`RUN_TO_END` in `tests/qemu_smoke.py`).

### Plan (a proposal; the kernel track decides)

One of these:

- **In libmind.** A program whose launcher's endpoint is gone ends at its next output, with a nonzero status, as a Unix program does on a hang-up. A program that must outlive its console (none is known) could opt out.
- **In the three tools.** libmind says whether the output still reaches the launcher (`mind::output::open()`). `check`, `bench` and `kbench` look at it between rows, write the log they have, and say in it that they stopped because their console closed.

### Acceptance criteria

In QEMU, `kbench` started from wm's menu ends within one row after its window is closed, and the kernel's log shows it. The wm suite's walk then closes the window without waiting, and `RUN_TO_END` is empty.

## The aarch64 boot check waits for `[BLOCKSTORE] READY` on the serial line, and it no longer comes there

**Recorded by:** the tools track (`APP`), 2026-10-11, from its local gate (the branch at b518d36 merged with `main` at dde9418).

### Problem

The group "aarch64: boot and fault containment" (`tests/aarch64_smoke.py`) fails on `main` itself (dde9418, built for aarch64 on its own):

- It reads the serial line until `[INIT] READY` and then requires `[BLOCKSTORE] READY BLOCKS=0 NAMES=0 SECTORS=1/16384 CORRUPT=0 DAMAGED=0`.
- Since 251-STO-0013 the block store first logs `[BLOCKSTORE] INDEX: 4681 SLOTS (255 KiB) FOR 16384 SECTORS`; its `READY` line never reaches the serial line, neither before `[INIT] READY` nor in 180 s of waiting for it (1 CPU and 4 CPUs alike, three runs).
- In the QEMU suites on aarch64 the same `READY` line is there: the store suite reads it with `logs <blockstore's PID>`, and every store check passes.
- Most likely the mount now ends after `init`'s `READY`, and lines printed after that do not go to the serial line in this test, which starts no shell. The vvfat crash (000-KRN-0067) may have hidden this in earlier gates, since it fails the same group.

### Plan (a proposal; the kernel track decides)

- Check the block store's `READY` where its output is kept rather than on the serial line, or wait for it before `[INIT] READY` is taken as the end of the boot.
- If the line should reach the serial line, find why it does not since the index grows with the medium (with the storage track, `requests-STO.md` names this request).

### Acceptance criteria

The group passes on `main` and on a branch merged with it, with the block store's `READY` checked.

## Boot images and `init`'s grants for `effector` and `effector_gw` (750)

**Recorded by:** the Effector agent track (EFF), 2026-10-09, for main task [750](750-effector-agent.md) at the maintainer's request.

### Problem

The Effector agent is two boot services:
- `effector` speaks to the server and parses its input;
- `effector_gw` acts on the machine within the owner's policy (MC-11.11).

The list of boot images is part of the ABI, and each service's grants are an arm of `Init::start`.

### Plan (a proposal; the kernel track decides)

- Two boot images in `common/abi.rs` (`BOOT_IMAGES`, `BOOT_SERVICES`), with the build, the signed manifest and the suites' service list.
- Both are off unless the owner enables them in `data/services.txt` (173-KRN-0035).
- **`effector` gets:**
  - `rtc`;
  - `tls` (`SLOT_TLS`);
  - a `parse` client (`SLOT_PARSE`, 109);
  - the `netpolicy` flow for program `effector` (renewable, when `NET` adds that);
  - a read-only `vfs` client scoped to `data/effector/`;
  - its own private directory `system/effector` for the token, as 351-KRN-0040 gives `keystore` its own;
  - an endpoint to `effector_gw`.
  - Nothing else.
- **`effector_gw` gets:**
  - a lifecycle client (the request below);
  - the log read badge;
  - `sysinfo`;
  - a writable `vfs` client scoped to `data/effector/` and `data/apps/` (the request below);
  - read-only clients for `log:` and the file roots;
  - a client of `updater` (requests-UPD);
  - an `init` client badged `BADGE_REBOOT`;
  - its own private directory `system/effector_gw` for the command journal;
  - screen capture, for the policy's `screenshot`.
  - No `tls`, no flow, no `parse` client, no device key. The test account's spawn rights are 501's, requested when 501-ASR-0009 starts.
- **The ABI change** goes with `libmind`, `docs/api` and the `isolation` suite, as AGENTS.md says.

### Acceptance criteria

In QEMU, with both enabled, each service holds exactly the listed grants (the `isolation` suite's checks). With them disabled, neither starts.

## A writable `vfs` client scoped to a service's directories (750)

**Recorded by:** the Effector agent track (EFF), 2026-10-09, for main task [750](750-effector-agent.md) at the maintainer's request.

### Problem

`vfs_server` lets only the shell's `BADGE_USER` client write, and services get read-only clients. The Effector gateway must write:
- its audit trail, which the owner reads;
- delivered files;
- installed applications, which the loader must be able to read.

It must not be able to write anywhere else on `data/`. A private directory in `system/` (351-KRN-0040, 108-KRN-0041) covers the journal, but not these: other programs, the shell and the loader must read them.

### Plan (a proposal; the kernel track, which owns `vfs_server` since 175, decides)

- A write grant limited to named directories, made by `init` from the service's arm (or `vfs.wit`'s `scope` applied to writes).
- Rename within the scope, and flush.
- A path outside the scope is refused with the same error as a missing file.

### Acceptance criteria

A service with a scoped write client creates, renames and flushes files under its directories, and is refused everywhere else, in a new `isolation` case.

## A lifecycle client limited to named services (750)

**Recorded by:** the Effector agent track (EFF), 2026-10-09, for main task [750](750-effector-agent.md) at the maintainer's request.

### Problem

`init`'s lifecycle client (`BADGE_LIFECYCLE`) may start, stop and restart every boot service. The Effector gateway needs that for the few services the owner names in its policy. Holding the whole authority would make it a broad retained authority that needs a justification (MC-3.7). An attenuated grant is the better answer (MC-3.4).

### Plan (a proposal; the kernel track decides)

- `init` gives the gateway a lifecycle client that serves `run`, `stop` and `restart` only for the boot services in a set:
  - fixed in the gateway's arm; or
  - read by `init` from the owner's configuration (173).
- `list` is allowed. `stop-task`, `reboot` and the services `init`, `shell`, `effector` and `effector_gw` are always refused.

### Acceptance criteria

In the `isolation` suite, the gateway's client restarts a service in its set and is refused for one outside it, for `init` and for itself.

## A PS/2 key press lost once on its way to `wm` (x86, after 000-KRN-0083)

**Recorded by:** the tools track (`APP`), 2026-10-11, from its local gate (the branch at bd4bc48 merged with `main` at f7e7a8a).

### Problem

The QEMU `wm` suite (x86, 4 CPUs) failed once in "x86: keys, shell, tools":

- It typed `reboot` and Enter into the shell's window with QEMU's `sendkey`, a key every 80 ms.
- `wm` logs a state line for each key press it reads (`mind::input::read_input`), and logged 6 for the 7 presses.
- So one press did not reach `wm`. The shell never asked `RESTART THE MACHINE? (Y/N)`.

The same build passed the whole suite when it ran again. Nothing in the tools track's change touches the path from `ps2_kbd` to `wm`. `main` had just brought the 1 ms tick (000-KRN-0083).

### Plan (a proposal; the kernel track decides)

- Look at where a press can be dropped between `ps2_kbd` and a listener, for example a full queue or a press and its release in one read.
- If it can, count the drops where the log shows them.

### Acceptance criteria

The `wm` suite types its lines with no press lost in repeated runs, or a drop is logged where it happens.

### Progress (the kernel track, 2026-10-14)

No cause found yet. What was ruled out, and what is left:

- **QEMU's PS/2 queue.** QEMU 8.2.2 keeps at most 16 bytes of keyboard data and drops what comes beyond them (`ps2_queue` in `hw/input/ps2.c`). With the translated scan code set a key is one byte down and one up, so `reboot` and Enter are 14 bytes: they fit even if the driver read nothing meanwhile.
- **The interrupt.** IRQ 1 goes through the 8259 PIC. The kernel masks the line until `ps2_kbd` acks it, and a masked 8259 still latches an edge (QEMU's `pic_set_irq` too), so a byte that arrives between the driver's last read and its ack still interrupts once the line is unmasked.
- **`ps2_kbd` and the decoder** read every byte while the controller has one, and only Pause's six bytes are skipped.
- **Where the kernel can drop a key** on its way to the task in front:
  - a full ring of 64 events (`INPUT_QUEUE`) gives up its oldest;
  - a change of focus empties both tasks' rings (`focus`).

  Neither fits one press in seven while `wm` keeps the focus.
- **Asked of the tools track if it fails again:** the run's serial log and `wm`'s state lines around the lost key, and whether `wm` logs one line for each event it reads or one for each read. If it can read two presses in one call, "6 lines for 7 presses" may be two presses in one line, and the press that went missing may be the Enter the shell needed.

## A slot for the Wi-Fi station's configuration client (550-APP-0033)

**Recorded by:** the tools track (APP), 2026-10-12, for `wifi` and the station's configuration interface proposed to `NET` (requests-NET.md, "`idl/wifi.wit`: what `wifi` needs of the station").

### Problem

`wifi` will talk to the network track's 802.11 station (`wlan`) through its configuration interface. A program gets a service's client only in a fixed slot its launcher lends, and there is none for this one.

### Plan (a proposal; the kernel track decides)

- `SLOT_WIFI` and `REQUEST_WIFI`, as `SLOT_BATTERY` and `REQUEST_BATTERY` came (211-KRN-0090).
- `init` gives the shell the station's configuration client with the badge that changes settings, once `wlan` runs.
- The shell lends it to a program that asks for `REQUEST_WIFI`; `wm` asks for it too and passes it on, as it passes the camera.

### Acceptance criteria

With the station (or a stand-in service of the agreed interface in QEMU), `wifi` started from the shell and from `wm` holds the client in `SLOT_WIFI`, and `caps` shows it there.

## A read-only client of a network card for a program, for `wm`'s background (000-NET-0015, 000-APP-0049)

**Recorded by:** the network track (`NET`), 2026-10-12, for [000-NET-0015](000-NET-0015-a-card-read-only-client.md) and the tools track's [000-APP-0049](000-APP-0049-network-traffic-on-wms-background.md).

### Problem

`idl/net.wit` 1.3 gives a card's driver a client that only reads the card: with `BADGE_COUNTERS` (`mind::network`) it may call `info`, `counters` and `offloads`, and nothing that sends, receives or lends a ring. Only a holder of a client with `CAP_BADGE` can mint it, and nobody that would lend it to a program has one: `init` lends the shell's `SLOT_NET` copy of `virtio_net` with `CLIENT` rights, and no program can ask for such a client.

### Plan (a proposal; the kernel track decides)

- **The client for the shell to lend.** Either:
  - `init` gives the shell's `SLOT_NET` copy `CAP_BADGE` too, as it gives `netstack` its own endpoint, so the shell mints the read-only copy itself; or
  - `init` mints the read-only client for the shell in a slot the kernel track chooses.
- **A request flag,** for example `REQUEST_NET_COUNTERS`, for which the shell lends the read-only client to a program in `SLOT_NET` (programs do not hold `SLOT_NET` otherwise). The shell's part is the tools track's, in 000-APP-0049.
- `wm` would hold one more lent client: the launch session's limit of 5 grants ("A launch session holds as many grants as there are launch slots" above) may come first.

### Acceptance criteria

A program that asks for the flag holds the read-only client in `SLOT_NET`: it reads the counters, and `send`, `receive` and `attach` answer `denied`.

## The Wi-Fi station in the boot services (550-NET-0018)

**Recorded by:** the network track (`NET`), 2026-10-12, for [550-NET-0018](550-NET-0018-the-station-service.md).

### Problem

The station service `wlan` (550-NET-0018) sits between a radio that serves `idl/wlanhw.wit` and the network stack. The radio is `bcm_wifi` on the MacBook Pro (550-DRV-0027), and a stand-in, `wlansim`, in QEMU. Only `init` can start it and hand out its clients, and the stack has slots for two cards only (`SLOT_DEV0`, `SLOT_DEV1`).

### Plan (a proposal; the kernel track decides)

- **Starting it.** `init` starts `wlan` when a radio service runs. On the MacBook Pro the radio is `bcm_wifi` as a frame service. In the `net` suite's image it is `wlansim`, which the test puts on the boot disk; a boot entry or a file could name it.
- **The radio's client.** `init` lends the radio's endpoint, the `idl/wlanhw.wit` endpoint of `bcm_wifi` (or of `wlansim`), to `wlan` alone, in slot 2 for example (MC-2.3; the drivers track asks the same, 550-DRV-0027). Nothing else gets a client of it.
- **The station's clients:**
  - an unbadged client to `netstack` as its third card, in a slot the kernel track chooses: the network track then raises `netstack`'s `CARDS`;
  - one with `BADGE_WIFI_CHANGE` (`mind::network`) to the shell. That is the client the tools track asked to lend to `wifi` ("A slot for the Wi-Fi station's configuration client" above).
- **`keystore`'s client for the station** (added 2026-10-12, for [550-NET-0024](550-NET-0024-remembered-wi-fi-networks.md)): a client of `keystore` with `BADGE_KEY_STATION` (`mind::network`, value 2) to `wlan` alone, in slot 3. With it the station has `keystore` keep a remembered network's PMK and asks it for each handshake's keys (`idl/keystore.wit` 1.1). Without it, `remember` and `forget` answer `unsupported`.
- **Restarts.** `wlan` restarts like the other services. The stack sees its card come back as it sees `virtio_net`'s.

### Acceptance criteria

In the `net` suite with `wlansim` on the disk, `wlan` runs, holds the radio's client and `keystore`'s, and `netstack` finds three cards. `stat caps` shows the station's clients where the plan puts them.

### Progress (2026-10-12)

**The drivers track, 2026-10-14 (550-DRV-0027):** `bcm_wifi`'s side is written: it serves `idl/wlanhw.wit` 1.0 on `SLOT_SERVICE` when it is given one, bringing the radio up at boot, and without it the core stays in reset as before. It is on `claude/ASR-DRV` and `fast-test` (3353c57d) and reaches `main` after its gate. So `init` needs only to give `bcm_wifi` a server endpoint, as it does for the other drivers, and lend its client to `wlan` alone.

The kernel track prepared 550-KRN-0097 (requests-NET.md on `main`). On the network branch, `netstack` now reads slot 4 as its third card (`CARDS` = 3), and `wlansim`'s network moved to 10.0.4.0/24 (the station's lease 10.0.4.15, its gateway 10.0.4.1), apart from the second card's 10.0.3.0/24 in the same suite. The join check of the `net` suite comes from the network track once 550-KRN-0097 is on `main`.

**On `main` since aedea68 (2026-10-14):** the code of 550-NET-0017 and 550-NET-0018 (`wpa`, `dot11`, `wlan`, `tests/wlansim`, `idl/wifi.wit`), `netstack`'s slot 4, and `keystore.wit` 1.1 for the station's client (550-NET-0024), gated on x86 and aarch64. 550-KRN-0097 can follow.

## The right to badge the shell's TLS client (351-NET-0022)

**Recorded by:** the network track (`NET`), 2026-10-12, for [351-NET-0022](351-NET-0022-tls-without-the-device-certificate.md).

### Problem

`tls` now refuses the device certificate to a client badged `BADGE_TLS_NO_DEVICE` (`idl/tls.wit` 1.2). The shell lends its own unbadged copy to programs (351-APP-0017), and it cannot make a badged one: `init` lends `SLOT_TLS` without `CAP_BADGE` (000-KRN-0073).

### Plan (a proposal; the kernel track decides)

- **The shell's copy.** `init` gives the shell's `SLOT_TLS` copy `CAP_BADGE` too. The shell then mints a `BADGE_TLS_NO_DEVICE` copy for each program it lends to (the tools track's part, requests-APP.md).
- **Or init mints it.** Alternatively, `init` mints the badged client for the shell in a slot of the kernel track's choosing.
- **The updater.** `init`'s client for `updater` may be the badged kind, unless the update server asks for the device's certificate.

### Acceptance criteria

`stat caps` shows the shell's TLS client with the right to badge (or the second client). The `tls` suite's check comes with the tools track's lending.

## The SSH service in the boot services (351-NET-0027)

**Recorded by:** the network track (`NET`), 2026-10-14, for [351-NET-0027](351-NET-0027-the-ssh-service.md).

### Problem

The service `ssh` (351-NET-0027) reads release files over SFTP for programs, as `tls` runs HTTPS for them (351-NET-0002). Its logins are signed by `keystore`, so the device key never leaves it (MC-11.9). Only `init` can start the service and hand out its clients, and no slot or request flag names it yet.

### Plan (a proposal; the kernel track decides)

- **Starting it.** `init` starts `ssh` (`ssh.elf`, built from `ssh_service/` since 2026-10-14) after `keystore` and the network stack, as it starts `tls`.
- **`keystore`'s client.** `init` mints a client of `keystore` with `BADGE_KEY_SSH` (`mind::network`, value 3) and lends it to `ssh` alone, in slot 4 as `tls` holds `BADGE_KEY_SIGNER`. With it, `ssh` may ask for `ssh-login` signatures only (`idl/keystore.wit` 1.2, 351-NET-0027); nothing else gets that badge.
- **The log.** `ssh` gets the log client, for its record of sessions, refusals and pin mismatches.
- **Its clients:**
  - the shell's, in a new fixed slot (`SLOT_SSH`), for the `download` command and for lending;
  - for programs, a request flag (`REQUEST_SSH`) that lends the launcher's client in the program's same slot, for a program that also gets a flow grant, as `REQUEST_TLS` does. A new slot in `LAUNCH_SLOTS` is an ABI change, so it is the kernel track's.
- **Restarts.** `ssh` restarts like `tls`.

### Acceptance criteria

In the `net` suite, `ssh` runs and holds `keystore`'s client with `BADGE_KEY_SSH`, and `stat caps` shows the shell's client in `SLOT_SSH`. The SFTP check of the `net` suite comes from the network track.

## A loader client for the updater's parsers (109-UPD-0017, for 109-NET-0010)

**Recorded by:** the update track (`UPD`), 2026-10-14, for [109-UPD-0017](109-UPD-0017-the-updaters-checks-in-their-own-parser.md), from the kernel track's request in requests-UPD.md.

### Problem

The updater now parses each check's channel, manifest and heads in a `parse` helper of the check's own, as `download` does (109-NET-0010), when it holds a loader client in `SLOT_LOADER`. `init` gives it none, and no endpoint quota, so it uses the shared service.

### Plan (a proposal; the kernel track decides)

- **A loader client that only starts helpers.** For example a badge on `loader`'s clients (`loader.wit` minor) with which only `helper` is served, and nothing that starts a program.
- **`init` lends it to the updater** in `SLOT_LOADER`, with an endpoint quota of 1: the session's endpoint, which the helper serves.

### Acceptance criteria

In the `update` suite, the updater's check runs its parsing in a helper that holds only its endpoint (`stat caps`), and the helper is gone when the check ends. A `program` call with the updater's loader client is refused.

## The shell's audio client that may change the levels (000-APP-0076, 000-APP-0077)

**Recorded by:** the tools track (APP), 2026-10-14, at the maintainer's request: "the output volume and the microphone's sensitivity need settings".

### Problem

`audio.wit` is asked for the output volume and the microphone's gain, and for setting them only with a badge (requests-DRV.md, "The output volume and the microphone's sensitivity in `audio.wit`").

- Today `init` lends the shell a plain client of `audio_gw` in `SLOT_AUDIO` (`init/src/main.rs`, the shell's grants), as it lends `tts` one.
- Programs get their own plain client from the loader's standard grants (`loader/src/main.rs`:108).

### Plan (a proposal; the kernel track decides)

- **A badge that changes the levels,** for example `BADGE_AUDIO_LEVELS` in `mind::audio`, as the clock's `BADGE_SET` (211-KRN-0051):
  - `init` mints it on the shell's client in `SLOT_AUDIO` only;
  - `tts` and the loader's grants keep the plain client, so no program the shell starts gets the badge.

### Acceptance criteria

`caps` shows the shell's audio client with the badge, and the client of a program the shell starts (`say`) without it. With `audio.wit` 1.2, `say` cannot change the volume and the shell can.

