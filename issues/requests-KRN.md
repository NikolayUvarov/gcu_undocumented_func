# Requests for the kernel track (KRN), not numbered yet

**Owner:** kernel track · **Status:** open (12 requests waiting, 2026-10-12, three of them for 750, one for 550-APP-0033, one for 109-NET-0010, one for 000-NET-0015 and 000-APP-0049; a drive that comes back checked to hold the same volumes became [000-KRN-0079](../issues-done/000-KRN-0079-a-returning-drive-is-checked-to-be-the-same-medium.done), the BCM4331's core stopped before its bus mastering became [550-KRN-0082](../issues-done/550-KRN-0082-the-bcm4331-core-stopped-before-its-bus-mastering.done), the badge right became [000-KRN-0073](../issues-done/000-KRN-0073-a-badge-set-only-with-the-right-to-set-it.done), the IDL fuzzer in CI became [000-KRN-0069](../issues-done/000-KRN-0069-the-idl-fuzzer-in-ci.done), bus mastering off at boot [550-KRN-0070](../issues-done/550-KRN-0070-bus-mastering-off-until-a-driver-has-the-device.done), blockstore's quota [251-KRN-0071](../issues-done/251-KRN-0071-a-memory-quota-for-blockstore.done), the FAT free count [251-KRN-0072](../issues-done/251-KRN-0072-free-clusters-counted-a-fat-sector-at-a-time.done); QEMU's vvfat crash in the aarch64 boot suite is [000-KRN-0067](../issues-done/000-KRN-0067-the-aarch64-boot-test-on-pcu.done))

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

## A program started with no standard clients, for a parser per session (109-NET-0010)

**Recorded by:** the network track (`NET`), 2026-10-12, for [109-NET-0010](109-NET-0010-a-parser-per-session.md).

### Problem

B.6 gives each session its own parser. Today one `parse` process serves every program. Its successor is a `parse` started for one session (a download, an update check) and ended with it, holding only its own endpoint. `loader` cannot start such a process: every program it starts gets the standard clients (`loader/src/main.rs`, slots 2 to 6: `rtc`, `vfs_server`, `audio_gw`, the loader's spawn privilege, `tts`), and "the standard grants cannot be replaced".

### Plan (a proposal; the kernel track decides)

- A launch without the standard grants: a flag on a launch session (`loader.wit`, a minor version), allowed only to a holder the kernel track chooses.
- The launcher gets the child's service endpoint back as a client, and the child holds its own endpoint and nothing else (`stat caps` shows it).
- The child ends with its launcher's session: when the launcher ends, or when it stops the child.
- Which programs may start such a parser (`download`, the updater) is the network track's part (109-NET-0010), with the request flag it needs from `KRN` and `APP`.

### Acceptance criteria

A test program starts a copy of `parse` this way. The copy's capabilities are its endpoint alone, the program gets an answer from it, and the copy ends when the program does.

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
