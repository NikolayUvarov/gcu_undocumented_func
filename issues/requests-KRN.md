# Requests for the kernel track (KRN), not numbered yet

**Owner:** kernel track · **Status:** open (6 requests waiting, 2026-10-12, three of them for 750; a drive that comes back checked to hold the same volumes became [000-KRN-0079](../issues-done/000-KRN-0079-a-returning-drive-is-checked-to-be-the-same-medium.done), the BCM4331's core stopped before its bus mastering became [550-KRN-0082](../issues-done/550-KRN-0082-the-bcm4331-core-stopped-before-its-bus-mastering.done), the badge right became [000-KRN-0073](../issues-done/000-KRN-0073-a-badge-set-only-with-the-right-to-set-it.done), the IDL fuzzer in CI became [000-KRN-0069](../issues-done/000-KRN-0069-the-idl-fuzzer-in-ci.done), bus mastering off at boot [550-KRN-0070](../issues-done/550-KRN-0070-bus-mastering-off-until-a-driver-has-the-device.done), blockstore's quota [251-KRN-0071](../issues-done/251-KRN-0071-a-memory-quota-for-blockstore.done), the FAT free count [251-KRN-0072](../issues-done/251-KRN-0072-free-clusters-counted-a-fat-sector-at-a-time.done); QEMU's vvfat crash in the aarch64 boot suite is [000-KRN-0067](../issues-done/000-KRN-0067-the-aarch64-boot-test-on-pcu.done))

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

