# Requests for the kernel track (KRN), not numbered yet

**Owner:** kernel track · **Status:** open (6 requests waiting, 2026-10-14, three of them for 750; the Wi-Fi station in the boot services, `keystore`'s client for it and the station's configuration slot became [550-KRN-0097](550-KRN-0097-the-wifi-station-in-the-boot-services.md), the read-only card client and the badged TLS client [000-KRN-0109](000-KRN-0109-the-shells-clients-may-badge-for-programs.md); a launch session holding every launch slot became [000-KRN-0101](000-KRN-0101-a-launch-session-holds-every-launch-slot.md), `init` saying whether it confirmed a trial boot [000-KRN-0102](000-KRN-0102-init-says-how-the-boot-went.md), the tools stopping when their console is gone [176-KRN-0103](176-KRN-0103-the-tools-stop-when-their-console-is-gone.md); a drive that comes back checked to hold the same volumes became [000-KRN-0079](../issues-done/000-KRN-0079-a-returning-drive-is-checked-to-be-the-same-medium.done), the BCM4331's core stopped before its bus mastering became [550-KRN-0082](../issues-done/550-KRN-0082-the-bcm4331-core-stopped-before-its-bus-mastering.done), the badge right became [000-KRN-0073](../issues-done/000-KRN-0073-a-badge-set-only-with-the-right-to-set-it.done), the IDL fuzzer in CI became [000-KRN-0069](../issues-done/000-KRN-0069-the-idl-fuzzer-in-ci.done), bus mastering off at boot [550-KRN-0070](../issues-done/550-KRN-0070-bus-mastering-off-until-a-driver-has-the-device.done), blockstore's quota [251-KRN-0071](../issues-done/251-KRN-0071-a-memory-quota-for-blockstore.done), the FAT free count [251-KRN-0072](../issues-done/251-KRN-0072-free-clusters-counted-a-fat-sector-at-a-time.done); QEMU's vvfat crash in the aarch64 boot suite is [000-KRN-0067](../issues-done/000-KRN-0067-the-aarch64-boot-test-on-pcu.done))

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

