# Requests for the kernel track (KRN), not numbered yet

**Owner:** kernel track · **Status:** open (10 requests waiting, 2026-10-12, three of them for 750; a drive that comes back checked to hold the same volumes became [000-KRN-0079](../issues-done/000-KRN-0079-a-returning-drive-is-checked-to-be-the-same-medium.done), the BCM4331's core stopped before its bus mastering became [550-KRN-0082](../issues-done/550-KRN-0082-the-bcm4331-core-stopped-before-its-bus-mastering.done), the badge right became [000-KRN-0073](../issues-done/000-KRN-0073-a-badge-set-only-with-the-right-to-set-it.done), the IDL fuzzer in CI became [000-KRN-0069](../issues-done/000-KRN-0069-the-idl-fuzzer-in-ci.done), bus mastering off at boot [550-KRN-0070](../issues-done/550-KRN-0070-bus-mastering-off-until-a-driver-has-the-device.done), blockstore's quota [251-KRN-0071](../issues-done/251-KRN-0071-a-memory-quota-for-blockstore.done), the FAT free count [251-KRN-0072](../issues-done/251-KRN-0072-free-clusters-counted-a-fat-sector-at-a-time.done); QEMU's vvfat crash in the aarch64 boot suite is [000-KRN-0067](../issues-done/000-KRN-0067-the-aarch64-boot-test-on-pcu.done))

The kernel track numbers its own tasks (`NNN-KRN-MMMM`), so requests from other tracks wait here. The kernel track turns each into a task and removes it from this file. The file is kept while empty because other issues link to it; a new request goes below this line.

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

## `bcm_wifi` gets DMA memory and its interrupt (550-DRV-0025)

**Recorded by:** the drivers track (`DRV`), 2026-10-12, ahead of stage 4 of the MacBook Pro's Wi-Fi, so that it can be done while stage 3 is written.

### Problem

Stage 2 ran Broadcom's microcode in the BCM4331 ([550-DRV-0023](../issues-done/550-DRV-0023-bcm4331-microcode-runs.done)). Stage 3 sets up the PHY and the radio through registers only. Stage 4 receives frames: the 802.11 core's DMA engines write them into rings in memory and raise an interrupt. `init` gives `bcm_wifi` BAR0 and a read-only VFS client, nothing else.

### Plan (a proposal; the kernel track decides)

In `init`'s `bcm_wifi` arm, as for `ahci` and `usb_host`:
- `SLOT_MEM`: one physically contiguous DMA region of 512 KiB (a receive ring of 64-bit descriptors, its buffers, and a transmit ring later), below 4 GiB;
- `SLOT_IRQ`: the function's MSI (capability at 0x48, one vector, 64-bit), or its INTx line where MSI is not available.

### Acceptance criteria

On the MacBook Pro, `bcm_wifi` logs the region's physical address and size and that it can wait on its interrupt. Nothing is started by the grant itself: the core stays idle until the driver sets up its rings.
