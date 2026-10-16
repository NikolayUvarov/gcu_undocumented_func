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

## CPU-budget threat-model wording (for 000-KRN-0105)

**Recorded by:** ASTRA auditor-fixer, 2026-10-15; MC-12.1, MC-12.3, MC-12.9.

The [hardware audit](../issues-audit/2026-10-15-hardware-audit.md), H08, found
another stale profile statement for [000-KRN-0105](000-KRN-0105-root-documents-agree-with-the-code.md):
`docs/profile/threat-model.md` says there are no CPU budgets and only
round-robin sharing. At the reviewed `1aea9100` candidate, `scheduler.rs`
implements budget exhaustion and period renewal (`Scheduler::ready`, `select`),
system-band precedence and `SCHED_SET`; new tasks default to unlimited.
`docs/profile/README.md` correctly marks MC-5.1–5.5 partial, and `clocks.md`
states the ten-millisecond slice limitation. The `busy` suite checks a 20/100
budget with an observed 12–35% range; that is not a guarantee of exactly 20%.

Please include this paragraph in the existing documentation task. Proposed
wording: complete protection against CPU denial of service is not claimed;
per-task budgets exist, default to unlimited and are enforced at scheduling
points, including the end of each ten-millisecond slice. Ready system-band
tasks take precedence over applications. Admission control, deadline/jitter
bounds and automatic service CPU-budget assignment by `init` are not provided.
Retain the limitations of [038](../issues-done/038-scheduling-budgets.done).

Acceptance: the threat-model paragraph agrees with the implementation and
the other profile documents without upgrading their partial guarantee. This
request changes no scheduler policy or existing test tolerance.


## Realtek staging integration assigned to the ASTRA auditor

The maintainer's 2026-10-15 network assignment is tracked in
[213-KRN-0114](213-KRN-0114-realtek-driver-startup.md). ASTRA imports the kernel
owner's original ABI 10/WLAN startup commits, then adds checked non-mastering
BAR/start control, Realtek boot grants and the ABI 11 transition. This is the
bounded integration dependency of DRV-0031/0032, not ownership of the remaining
kernel backlog. No change is sent to the kernel owner's branch. Physical
MacBook/Ryzen evidence and the deferred ARM gate remain open.

## Safe-start services need a prepared plan before Platform is dropped

**Recorded by:** ASTRA auditor-fixer, 2026-10-15; H10 in the hardware audit.

`init` skips `SAFE_LEFT_OUT` before constructing capability plans. A later
`svc start bcm_wifi` (also the new RTL8852BE board stage) runs hardware discovery
after Platform was dropped and cannot start the device. Prepare narrowly scoped
saved plans without activating these devices, or expose an explicit supported
recovery path. Preserve safe start and the end of bootstrap authority
(MC-3.12, MC-6.3). Acceptance: a safe boot performs no radio startup; the operator
can explicitly start its omitted radio afterward without restoring Platform or
rebooting, or the UI clearly describes the deliberately unsupported operation.

## Device-bound platform power policy for the Ryzen WLAN driver

**Recorded by:** ASTRA auditor-fixer, 2026-10-16; dependency of 213-DRV-0032,
roadmap H / 213 and D / 550; MC-2.3, MC-3.3, MC-6.1, MC-12.1, MC-12.3.

The RTL8852BE channel path requires explicit platform SAR and antenna-gain
constraints, separately from firmware default tables and operator country.
The current native driver has neither a device-bound platform-policy result
nor a complete checked ACPI snapshot from which such a result could be derived.
`PLATFORM_ACPI_TABLE` already copies individual tables read-only during init;
`init/src/hardware.rs` writes their diagnostic copies to `log:acpi/`. That
diagnostic loop does not establish complete namespace evaluation, absence of
device-specific policy or a durable scoped authority after Platform is dropped.

Please number the smallest suitable KRN dependency and choose its interface:

- Supply a complete, bounded, validated snapshot of applicable AML tables and
  PCI identity/topology to a scoped consumer, or a typed device-bound result
  `KnownLimits / KnownAbsent / UnsupportedOrUnknown`. Do not restore general
  Platform privilege to the radio service.
- Distinguish an actually absent supported policy from inaccessible, truncated,
  malformed or unsupported input. Known absence may use a justified bounded
  analysis; this request does not demand a general AML interpreter. Unknown
  input must not silently become unlimited transmit power.
- Retain actual limits/antenna offsets, their source and policy version for
  the DRV consumer. Version changed ABI/IDL explicitly; document capability
  ownership, bounds and restart behavior. Intel/x86 is the priority; ARM
  execution remains deferred by the maintainer.

Pinned RTL8852B source looks up `WRDS`, `RWRD`, `RWSI`, `RWGS` and `RTAG` relative
to its WLAN ACPI device. A matching string in an unrelated scope is not a
policy. Read-only Ubuntu Ryzen operation `20261015233912.417877721_000172`
read DSDT and nine SSDTs: none contained those five names; PCI `03:00.0` had
no `firmware_node` entry. It did not evaluate AML or change the NIC. These
observations narrow the expected profile but are not a MIND runtime result.
Raw hashes and source details are recorded on ASTRA in
`hwdocs/rtl8852be-channel-stage.md`.

Acceptance: supported valid/absent fixtures have explicit device identity and
provenance; invalid, partial or unsupported snapshots remain distinguishable;
the driver cannot substitute an unrelated snapshot or infer permission from
an error; no ambient Platform capability is retained. Record actual Ryzen
evidence separately. DRV-0032 remains blocked on this request for native
transmit-policy admission; calibration/datapath development can continue
without enabling transmission.

## Native RTL8852BE radio endpoint after datapath admission

**Recorded by:** ASTRA auditor-fixer, 2026-10-16; dependency of 213-DRV-0032,
roadmap H / 213 and D / 550; MC-2.3, MC-3.3, MC-6.1, MC-12.1, MC-12.3.

The existing ASTRA-owned 213-KRN-0114 integration deliberately starts a
diagnostic: the `rtw89_wifi` init branch lends BAR2, retained DMA and a firmware
reader but no `SLOT_SERVICE`. `RADIOS` contains only `bcm_wifi` and `wlansim`.
The new driver-side data owner therefore cannot become a usable station by
itself. Please number the radio-service extension and coordinate activation
with DRV-0032 rather than advertising the diagnostic as a working radio.

The maintainer confirmed on 2026-10-16 that both target machines are physically
in Uzbekistan (`UZ`), and then requested switching among every country in the
verified regulatory database. Use UZ as this deployment's explicit initial
selection; do not turn it into an implicit worldwide default for other installs.

- Reserve the current private service's receive-capability slots 9 and 10 as
  empty, or coordinate an explicit driver-contract change. Its constructor
  checks both slots; the endpoint/configuration grants must not collide with
  deferred request ownership. Preserve the current log, BAR, DMA and VFS grants.
- Give the selected RTL8852BE instance its `wlanhw` server endpoint and lend
  its client only to `wlan`, preserving the original MacBook BCM4331 and QEMU
  selections. Do not alter their authority or replace a running radio merely
  because a second PCI device exists.
- Provide an explicit bounded operator country/channel configuration, with
  provenance separate from the device-bound platform policy requested above.
  A stored Ubuntu regulatory setting is not operator confirmation, and missing
  or invalid input must remain visible as unavailable radio service.
- Preserve retained DMA, verified non-mastering restart and safe-start rules;
  neither this endpoint nor its configuration grants ambient Platform access.
  Support a persisted, owner-scoped selection for all catalogued countries,
  coordinated with the NET/APP requests below; configuration data alone must
  not grant transmission. If an interface changes, version it and document
  the migration.
- Sequence the change with the actual persistent driver loop and its admitted
  capabilities. Initial driver support may advertise one explicitly admitted
  2.4 GHz / 20 MHz channel; other channels must be rejected, never reported as
  tuned. A running diagnostic process is not an endpoint-readiness signal.

Acceptance: fixtures cover no Realtek device, missing/invalid configuration,
driver initialization refusal, exact endpoint ownership and preserved BCM4331
and wlansim routing. The original Intel MacBook and Ryzen must separately
establish association, DHCP and ping under test load before physical readiness
is claimed. Driver DMA partitioning, active RX/TX and service dispatch stay in
DRV-0032; this request does not transfer their implementation to KRN.


### Country-selection integration requests (2026-10-16)

See [NET's policy/configuration request](requests-NET.md#wifi-country-selection-from-the-complete-verified-regulatory-catalog)
and [APP's selector request](requests-APP.md#wifi-country-and-channel-selector).
The current driver is limited to a single admitted 2.4 GHz/20 MHz channel; a
country selection must intersect radio capabilities and existing board/platform
constraints. A configuration change must never reuse an active admission made
under a different policy. Coordinate stop/restart/re-admission and bounded
failure reporting with the driver and station owners.
