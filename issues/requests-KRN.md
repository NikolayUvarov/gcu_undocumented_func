# Requests for the kernel track (KRN), not numbered yet

**Owner:** kernel track · **Status:** open (4 requests waiting: the block store's volume key kept by `keystore` and its endpoint with the badge right joined them on 2026-10-29; 2026-10-23: the PS/2 key press and the ASTRA branch's Realtek staging note; memory and CPUs for large models, for 262, became [262-KRN-0160](../issues-done/262-KRN-0160-large-private-memory-for-a-named-program.done) to [262-KRN-0164](../issues-done/262-KRN-0164-stacks-larger-than-64-kib.done) (0160, 0162 to 0164 done); the applications from the booted slot became [351-KRN-0151](351-KRN-0151-applications-from-the-booted-slot.md); the across field in absolute pointer events became [211-KRN-0150](../issues-done/211-KRN-0150-an-across-field-in-absolute-pointer-events.done) and a key code for PrintScreen [000-KRN-0149](../issues-done/000-KRN-0149-a-key-code-for-print-screen.done); the loader's helper on a disk with boot slots became [109-KRN-0133](109-KRN-0133-helpers-from-the-booted-slot.md), the loader's memory request above its quota [000-KRN-0134](../issues-done/000-KRN-0134-a-memory-request-above-the-loaders-quota.done); the safe start's prepared plans became [000-KRN-0131](000-KRN-0131-a-safe-start-prepares-what-it-leaves-out.md); the CPU-budget wording of the threat model became [000-KRN-0130](../issues-done/000-KRN-0130-the-threat-model-states-the-cpu-budgets.done); the SSH service became [351-KRN-0115](../issues-done/351-KRN-0115-the-ssh-service-in-the-boot-services.done), the shell's client of the updater [351-KRN-0116](351-KRN-0116-the-shells-client-of-the-updater.md), the updater's loader client [109-KRN-0117](109-KRN-0117-a-loader-client-that-only-starts-helpers.md), the shell's audio levels [000-KRN-0118](000-KRN-0118-the-shells-audio-client-may-change-the-levels.md), the Effector agent's boot images and grants [750-KRN-0119](../issues-done/750-KRN-0119-boot-images-and-grants-for-effector.done), its writable scopes [750-KRN-0120](../issues-done/750-KRN-0120-writable-scopes-for-a-service.done) and its lifecycle client [750-KRN-0121](../issues-done/750-KRN-0121-a-lifecycle-client-for-named-services.done), the RTL8852BE's platform power policy [213-KRN-0122](213-KRN-0122-platform-power-policy-for-the-wlan-device.md) and its radio endpoint [213-KRN-0123](213-KRN-0123-the-rtl8852be-radio-endpoint.md); the aarch64 boot check's wait for the block store's `READY` was done in [000-KRN-0067](../issues-done/000-KRN-0067-the-aarch64-boot-test-on-pcu.done); the Wi-Fi station in the boot services, `keystore`'s client for it and the station's configuration slot became [550-KRN-0097](../issues-done/550-KRN-0097-the-wifi-station-in-the-boot-services.done), the read-only card client and the badged TLS client [000-KRN-0109](../issues-done/000-KRN-0109-the-shells-clients-may-badge-for-programs.done); a launch session holding every launch slot became [000-KRN-0101](../issues-done/000-KRN-0101-a-launch-session-holds-every-launch-slot.done), `init` saying whether it confirmed a trial boot [000-KRN-0102](../issues-done/000-KRN-0102-init-says-how-the-boot-went.done), the tools stopping when their console is gone [176-KRN-0103](../issues-done/176-KRN-0103-the-tools-stop-when-their-console-is-gone.done); a drive that comes back checked to hold the same volumes became [000-KRN-0079](../issues-done/000-KRN-0079-a-returning-drive-is-checked-to-be-the-same-medium.done), the BCM4331's core stopped before its bus mastering became [550-KRN-0082](../issues-done/550-KRN-0082-the-bcm4331-core-stopped-before-its-bus-mastering.done), the badge right became [000-KRN-0073](../issues-done/000-KRN-0073-a-badge-set-only-with-the-right-to-set-it.done), the IDL fuzzer in CI became [000-KRN-0069](../issues-done/000-KRN-0069-the-idl-fuzzer-in-ci.done), bus mastering off at boot [550-KRN-0070](../issues-done/550-KRN-0070-bus-mastering-off-until-a-driver-has-the-device.done), blockstore's quota [251-KRN-0071](../issues-done/251-KRN-0071-a-memory-quota-for-blockstore.done), the FAT free count [251-KRN-0072](../issues-done/251-KRN-0072-free-clusters-counted-a-fat-sector-at-a-time.done); QEMU's vvfat crash in the aarch64 boot suite is [000-KRN-0067](../issues-done/000-KRN-0067-the-aarch64-boot-test-on-pcu.done))

The kernel track numbers its own tasks (`NNN-KRN-MMMM`), so requests from other tracks wait here. The kernel track turns each into a task and removes it from this file. The file is kept while empty because other issues link to it; a new request goes below this line.

The request on `bcm_wifi`'s DMA region and interrupt became [550-KRN-0091](550-KRN-0091-bcm-wifi-dma-and-its-interrupt.md) (2026-10-12).

The store's own partition (STO, for 307) became [307-KRN-0177](307-KRN-0177-the-stores-own-partition-classified-and-granted.md), and the model service `nnd` (APP, for 262-APP-0112) [262-KRN-0178](262-KRN-0178-nnd-as-a-boot-service.md) with the wait on a shared word [262-KRN-0179](262-KRN-0179-a-wait-on-a-shared-word.md) (2026-10-28).

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

## Realtek staging integration assigned to the ASTRA auditor

The maintainer's 2026-10-15 network assignment is tracked in
[213-KRN-0114](213-KRN-0114-realtek-driver-startup.md). ASTRA imports the kernel
owner's original ABI 10/WLAN startup commits, then adds checked non-mastering
BAR/start control, Realtek boot grants and the ABI 11 transition. This is the
bounded integration dependency of DRV-0031/0032, not ownership of the remaining
kernel backlog. No change is sent to the kernel owner's branch. Physical
MacBook/Ryzen evidence and the deferred ARM gate remain open.

## The block store's volume key kept by `keystore` (STO, for 308-STO-0022)

**Recorded by:** the storage track (STO), 2026-10-29, for [308-STO-0022](308-STO-0022-the-volume-key-held-by-keystore.md). `keystore` is the network track's, which the kernel session holds, so both parts are asked here.

### Problem

Since [308-STO-0021](../issues-done/308-STO-0021-sealed-records.done) every record of the block store is sealed, but the volume key lies in the superblock in the clear: whoever reads the medium opens the records. The store needs a place for its key that is not its medium.

### Plan (a proposal; the kernel session decides)

- **No new call in `keystore.wit`.** Its 1.4 program secrets (000-NET-0045, on `claude/youthful-mendel-mf1soy`) already keep what is needed: a value only its program reads, in `keystore`'s private directory, sealed under a key derived from the device key, and so under the TPM's seal once 351-NET-0006 is active. The store keeps its volume key as its secret `volume-<id>`; the superblock holds the id and a check, never the key.
- **What the store needs is a secrets client of its own,** as the loader gets one for a program (000-KRN-0182):
  - `init` asks `keystore` for it (`secrets-client`), names it (`secrets-name`), gives it to `blockstore` in a slot (3 is free there) and names the task (`secrets-adopt`);
  - `init` holds a `keystore` client that may do that, the loader's badge or one of its own.
- **A name no program can take.** The loader names a secrets client by the name a program runs by, so a program called `blockstore` in `data/apps/` would reach the store's secrets. The store's client needs a name no program run by the loader can have (a prefix the loader never gives, say `svc.blockstore`), or a namespace of services apart from programs.
- **Order and recovery (MC-6.8).** The store depends on `keystore`, never the other way round. `blockstore` asks for its key at mount; without an answer it stays up and answers every request with the reason, as with a medium it cannot mount (305-STO-0008). Nothing the system needs to start depends on either.
- **Without a client** (no `keystore` or no device key), the store makes no key for a medium that outlives a reset and says so. A RAM disk's key is kept in memory only.

### Acceptance criteria

- `blockstore` holds a secrets client under a name no loaded program can have, from `init` at boot (x86 and aarch64).
- A secret `blockstore` keeps through it is read back after a reboot. The storage track's suites check the rest once the store uses it.

## The block store's own endpoint with the badge right (STO, for 308-STO-0023)

**Recorded by:** the storage track (STO), 2026-10-29, for [308-STO-0023](308-STO-0023-memory-domains.md).

### Problem

A memory domain is worked in through a client the store mints for it from its own endpoint, with a badge from `0x100` up (`blockstore.wit` 2.2 `domain-client`), as `vfs_server` mints confined file clients and `keystore` secrets clients. `init` grants `blockstore` its endpoint in `SLOT_SERVICE` with `ALL` and without `CAP_BADGE` (`init/src/main.rs`, the `"blockstore"` arm), so the mint is refused. In QEMU, a domain is made, and then `blocks -d 1 put` gets no client.

### Plan

- **One line:** `grants.add(SLOT_SERVICE, self.server(&mut minted, "blockstore")?, ALL | CAP_BADGE);`, as for `vfs_server`.
- The badges `init` gives the store's clients stay below `0x100` (`mind::blockstore::FIRST_DOMAIN_CLIENT`). The store treats a badge from there up that it did not mint as one with no right.
- `docs/profile/bootstrap.md`'s row for `blockstore` would say it may badge its own clients.

### Acceptance criteria

The storage track's checks of domains pass (they are on `claude/relaxed-meitner-5bmhpz`, waiting for this):
- the `store` suite on aarch64, where RNDR gives the key;
- the `boot` suite's store disk on x86 with `+rdrand`, a domain forgotten across a reboot.

The storage track ran them with this line patched in locally, uncommitted, and reports the result in the landing request.

