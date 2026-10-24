# Requests for the kernel track (KRN), not numbered yet

**Owner:** kernel track · **Status:** open (2 requests waiting, 2026-10-23: the PS/2 key press and the ASTRA branch's Realtek staging note; memory and CPUs for large models, for 262, became [262-KRN-0160](../issues-done/262-KRN-0160-large-private-memory-for-a-named-program.done) to [262-KRN-0164](../issues-done/262-KRN-0164-stacks-larger-than-64-kib.done) (0160, 0162 to 0164 done); the applications from the booted slot became [351-KRN-0151](351-KRN-0151-applications-from-the-booted-slot.md); the across field in absolute pointer events became [211-KRN-0150](../issues-done/211-KRN-0150-an-across-field-in-absolute-pointer-events.done) and a key code for PrintScreen [000-KRN-0149](../issues-done/000-KRN-0149-a-key-code-for-print-screen.done); the loader's helper on a disk with boot slots became [109-KRN-0133](109-KRN-0133-helpers-from-the-booted-slot.md), the loader's memory request above its quota [000-KRN-0134](../issues-done/000-KRN-0134-a-memory-request-above-the-loaders-quota.done); the safe start's prepared plans became [000-KRN-0131](000-KRN-0131-a-safe-start-prepares-what-it-leaves-out.md); the CPU-budget wording of the threat model became [000-KRN-0130](../issues-done/000-KRN-0130-the-threat-model-states-the-cpu-budgets.done); the SSH service became [351-KRN-0115](../issues-done/351-KRN-0115-the-ssh-service-in-the-boot-services.done), the shell's client of the updater [351-KRN-0116](351-KRN-0116-the-shells-client-of-the-updater.md), the updater's loader client [109-KRN-0117](109-KRN-0117-a-loader-client-that-only-starts-helpers.md), the shell's audio levels [000-KRN-0118](000-KRN-0118-the-shells-audio-client-may-change-the-levels.md), the Effector agent's boot images and grants [750-KRN-0119](../issues-done/750-KRN-0119-boot-images-and-grants-for-effector.done), its writable scopes [750-KRN-0120](../issues-done/750-KRN-0120-writable-scopes-for-a-service.done) and its lifecycle client [750-KRN-0121](../issues-done/750-KRN-0121-a-lifecycle-client-for-named-services.done), the RTL8852BE's platform power policy [213-KRN-0122](213-KRN-0122-platform-power-policy-for-the-wlan-device.md) and its radio endpoint [213-KRN-0123](213-KRN-0123-the-rtl8852be-radio-endpoint.md); the aarch64 boot check's wait for the block store's `READY` was done in [000-KRN-0067](../issues-done/000-KRN-0067-the-aarch64-boot-test-on-pcu.done); the Wi-Fi station in the boot services, `keystore`'s client for it and the station's configuration slot became [550-KRN-0097](../issues-done/550-KRN-0097-the-wifi-station-in-the-boot-services.done), the read-only card client and the badged TLS client [000-KRN-0109](../issues-done/000-KRN-0109-the-shells-clients-may-badge-for-programs.done); a launch session holding every launch slot became [000-KRN-0101](../issues-done/000-KRN-0101-a-launch-session-holds-every-launch-slot.done), `init` saying whether it confirmed a trial boot [000-KRN-0102](../issues-done/000-KRN-0102-init-says-how-the-boot-went.done), the tools stopping when their console is gone [176-KRN-0103](../issues-done/176-KRN-0103-the-tools-stop-when-their-console-is-gone.done); a drive that comes back checked to hold the same volumes became [000-KRN-0079](../issues-done/000-KRN-0079-a-returning-drive-is-checked-to-be-the-same-medium.done), the BCM4331's core stopped before its bus mastering became [550-KRN-0082](../issues-done/550-KRN-0082-the-bcm4331-core-stopped-before-its-bus-mastering.done), the badge right became [000-KRN-0073](../issues-done/000-KRN-0073-a-badge-set-only-with-the-right-to-set-it.done), the IDL fuzzer in CI became [000-KRN-0069](../issues-done/000-KRN-0069-the-idl-fuzzer-in-ci.done), bus mastering off at boot [550-KRN-0070](../issues-done/550-KRN-0070-bus-mastering-off-until-a-driver-has-the-device.done), blockstore's quota [251-KRN-0071](../issues-done/251-KRN-0071-a-memory-quota-for-blockstore.done), the FAT free count [251-KRN-0072](../issues-done/251-KRN-0072-free-clusters-counted-a-fat-sector-at-a-time.done); QEMU's vvfat crash in the aarch64 boot suite is [000-KRN-0067](../issues-done/000-KRN-0067-the-aarch64-boot-test-on-pcu.done))

The kernel track numbers its own tasks (`NNN-KRN-MMMM`), so requests from other tracks wait here. The kernel track turns each into a task and removes it from this file. The file is kept while empty because other issues link to it; a new request goes below this line.

The request on `bcm_wifi`'s DMA region and interrupt became [550-KRN-0091](550-KRN-0091-bcm-wifi-dma-and-its-interrupt.md) (2026-10-12).

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
