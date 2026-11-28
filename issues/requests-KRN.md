# Requests for the kernel track (KRN), not numbered yet

**Owner:** kernel track · **Status:** open (3 requests waiting: programs larger than 8 MiB, APP's, 2026-11-27, whose step 1 became [262-KRN-0292](262-KRN-0292-programs-larger-than-8-mib.md) and step 2 waits here; the PS/2 key press and the ASTRA branch's Realtek staging note, 2026-10-23; the shell's owner client of `effector_gw` (APP's, 2026-11-14) became [801-KRN-0246](../issues-done/801-KRN-0246-the-shells-owner-client-of-the-effector-gateway.done); the `tools` suite in an aarch64 group (APP's, 2026-11-13) became [000-KRN-0240](../issues-done/000-KRN-0240-the-tools-suite-in-an-aarch64-group.done); a calendar clock for `blockstore` (STO's, 2026-11-13) became [312-KRN-0239](../issues-done/312-KRN-0239-a-calendar-clock-for-blockstore.done); the copy's partition on a real machine (STO's) became [307-KRN-0236](307-KRN-0236-the-copys-partition-on-a-real-machine.md), `init` starting the clipboard service (APP's) [000-KRN-0235](../issues-done/000-KRN-0235-init-starts-the-clipboard-service.done), and a program's own directory in the store (STO's) [311-KRN-0237](311-KRN-0237-a-programs-own-directory-in-the-store.md) (2026-11-12); APP's scope and memory for `tts`'s neural voice became [252-KRN-0230](../issues-done/252-KRN-0230-tts-reads-the-neural-voice.done) (2026-11-10); APP's wiping of `secret-line`'s binding buffer became [000-KRN-0226](../issues-done/000-KRN-0226-an-idl-clients-call-buffer-wiped-before-it-is-freed.done) (2026-11-09); STO's store client for the updater, 2026-11-08, became [351-KRN-0218](../issues-done/351-KRN-0218-the-updaters-store-client.done); STO's second store disk for the block store's copy became [309-KRN-0209](../issues-done/309-KRN-0209-a-second-store-disk-for-the-block-stores-copy.done) (2026-11-04); STO's store volumes after a restart of `storefs` became [311-KRN-0207](../issues-done/311-KRN-0207-mind-fs-opens-a-volumes-root-again-after-its-service-restarts.done) (2026-11-04); STO's client that may recover the volume key became [308-KRN-0204](../issues-done/308-KRN-0204-a-store-client-that-may-recover-the-volume-key.done), with [308-KRN-0205](../issues-done/308-KRN-0205-the-boot-waits-for-keystores-passphrase.done) and STO's [308-NET-0055](../issues-done/308-NET-0055-keystores-passphrase-without-a-tpm.done) (2026-11-04); `storefs`'s start, its files clients and a `conflict` error (STO, 2026-10-30) became [311-KRN-0189](../issues-done/311-KRN-0189-a-conflict-error-in-vfs-wit.done) and [311-KRN-0190](../issues-done/311-KRN-0190-storefs-at-boot-and-its-clients.done); the block store's volume key kept by `keystore` and its endpoint with the badge right (STO, 2026-10-29) became [308-KRN-0183](../issues-done/308-KRN-0183-a-secrets-client-for-blockstore.done) and [308-KRN-0184](../issues-done/308-KRN-0184-blockstore-badges-its-own-clients.done); memory and CPUs for large models, for 262, became [262-KRN-0160](../issues-done/262-KRN-0160-large-private-memory-for-a-named-program.done) to [262-KRN-0164](../issues-done/262-KRN-0164-stacks-larger-than-64-kib.done) (0160, 0162 to 0164 done); the applications from the booted slot became [351-KRN-0151](351-KRN-0151-applications-from-the-booted-slot.md); the across field in absolute pointer events became [211-KRN-0150](../issues-done/211-KRN-0150-an-across-field-in-absolute-pointer-events.done) and a key code for PrintScreen [000-KRN-0149](../issues-done/000-KRN-0149-a-key-code-for-print-screen.done); the loader's helper on a disk with boot slots became [109-KRN-0133](../issues-done/109-KRN-0133-helpers-from-the-booted-slot.done), the loader's memory request above its quota [000-KRN-0134](../issues-done/000-KRN-0134-a-memory-request-above-the-loaders-quota.done); the safe start's prepared plans became [000-KRN-0131](000-KRN-0131-a-safe-start-prepares-what-it-leaves-out.md); the CPU-budget wording of the threat model became [000-KRN-0130](../issues-done/000-KRN-0130-the-threat-model-states-the-cpu-budgets.done); the SSH service became [351-KRN-0115](../issues-done/351-KRN-0115-the-ssh-service-in-the-boot-services.done), the shell's client of the updater [351-KRN-0116](351-KRN-0116-the-shells-client-of-the-updater.md), the updater's loader client [109-KRN-0117](109-KRN-0117-a-loader-client-that-only-starts-helpers.md), the shell's audio levels [000-KRN-0118](000-KRN-0118-the-shells-audio-client-may-change-the-levels.md), the Effector agent's boot images and grants [750-KRN-0119](../issues-done/750-KRN-0119-boot-images-and-grants-for-effector.done), its writable scopes [750-KRN-0120](../issues-done/750-KRN-0120-writable-scopes-for-a-service.done) and its lifecycle client [750-KRN-0121](../issues-done/750-KRN-0121-a-lifecycle-client-for-named-services.done), the RTL8852BE's platform power policy [213-KRN-0122](213-KRN-0122-platform-power-policy-for-the-wlan-device.md) and its radio endpoint [213-KRN-0123](213-KRN-0123-the-rtl8852be-radio-endpoint.md); the aarch64 boot check's wait for the block store's `READY` was done in [000-KRN-0067](../issues-done/000-KRN-0067-the-aarch64-boot-test-on-pcu.done); the Wi-Fi station in the boot services, `keystore`'s client for it and the station's configuration slot became [550-KRN-0097](../issues-done/550-KRN-0097-the-wifi-station-in-the-boot-services.done), the read-only card client and the badged TLS client [000-KRN-0109](../issues-done/000-KRN-0109-the-shells-clients-may-badge-for-programs.done); a launch session holding every launch slot became [000-KRN-0101](../issues-done/000-KRN-0101-a-launch-session-holds-every-launch-slot.done), `init` saying whether it confirmed a trial boot [000-KRN-0102](../issues-done/000-KRN-0102-init-says-how-the-boot-went.done), the tools stopping when their console is gone [176-KRN-0103](../issues-done/176-KRN-0103-the-tools-stop-when-their-console-is-gone.done); a drive that comes back checked to hold the same volumes became [000-KRN-0079](../issues-done/000-KRN-0079-a-returning-drive-is-checked-to-be-the-same-medium.done), the BCM4331's core stopped before its bus mastering became [550-KRN-0082](../issues-done/550-KRN-0082-the-bcm4331-core-stopped-before-its-bus-mastering.done), the badge right became [000-KRN-0073](../issues-done/000-KRN-0073-a-badge-set-only-with-the-right-to-set-it.done), the IDL fuzzer in CI became [000-KRN-0069](../issues-done/000-KRN-0069-the-idl-fuzzer-in-ci.done), bus mastering off at boot [550-KRN-0070](../issues-done/550-KRN-0070-bus-mastering-off-until-a-driver-has-the-device.done), blockstore's quota [251-KRN-0071](../issues-done/251-KRN-0071-a-memory-quota-for-blockstore.done), the FAT free count [251-KRN-0072](../issues-done/251-KRN-0072-free-clusters-counted-a-fat-sector-at-a-time.done); QEMU's vvfat crash in the aarch64 boot suite is [000-KRN-0067](../issues-done/000-KRN-0067-the-aarch64-boot-test-on-pcu.done))

The kernel track numbers its own tasks (`NNN-KRN-MMMM`), so requests from other tracks wait here. The kernel track turns each into a task and removes it from this file. The file is kept while empty because other issues link to it; a new request goes below this line.

The request on `bcm_wifi`'s DMA region and interrupt became [550-KRN-0091](550-KRN-0091-bcm-wifi-dma-and-its-interrupt.md) (2026-10-12). NRL's request for its host checks in CI became [900-KRN-0174](900-KRN-0174-nrl-checks-in-ci.md) (2026-10-28).

APP's reading client of the updater lent to a program (for 100003-APP-0151) became [100003-KRN-0291](100003-KRN-0291-a-reading-client-of-the-updater-for-programs.md) (2026-11-28).

NET's station endpoint with the badge right (for 550-NET-0058) became [550-KRN-0213](../issues-done/550-KRN-0213-the-stations-endpoint-may-badge-its-reading-client.done) (2026-11-05).

The store's own partition (STO, for 307) became [307-KRN-0177](../issues-done/307-KRN-0177-the-stores-own-partition-classified-and-granted.done), and the model service `nnd` (APP, for 262-APP-0112) [262-KRN-0178](262-KRN-0178-nnd-as-a-boot-service.md) with the wait on a shared word [262-KRN-0179](262-KRN-0179-a-wait-on-a-shared-word.md) (2026-10-28).

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

## Programs larger than 8 MiB, and later threads with TLS (for large inference modules and for 264)

**Asked by:** the tools track (APP), 2026-11-27, at the maintainer's word: «если мы говорим об ограничении 8мб на бинарник, то оно кажется слишком маленьким, так как большие инференсеры для mind могут быть видимо намного больше. Это надо учитывать и готовится к тому, что мыслительные модули будут большими» · **For:** the cognitive modules to come ([262](262-model-runtime-on-the-machines-full-hardware.md), Article 8), and [264](264-ladybird-on-mind-core-a-port-kept-outside-the-os.md), Ladybird on MIND Core · **Constitution:** MC-3.3, MC-5.1, MC-12.4

**Problem.**
- `kernel/src/elf.rs` refuses an image whose loaded segments span more than 8 MiB ("INVALID ELF IMAGE OR ENTRY POINT"), and any `PT_TLS` or `PT_INTERP` ("ELF INTERPRETER/TLS NOT SUPPORTED").
- Today's largest program is `tls.elf`, 1.45 MB, so nothing has met the limit yet.
- **Measured** on 2026-11-27 in Ladybird's official package 2026.48.0 for amd64: its `WebContent` is 94 MB stripped (`.text` 75.7 MB) and carries a 15 KB TLS segment. An inference runtime with its kernels for several CPUs and accelerators will grow past 8 MiB as well.

**Asked:**
1. **Now: images larger than 8 MiB.** Taken on 2026-11-28 as [262-KRN-0292](262-KRN-0292-programs-larger-than-8-mib.md): one bound, `IMAGE_MAX` 512 MiB (ABI 40); the image still in one contiguous run and no pages shared, which wait with step 2.
   - A bound that a large module fits, for example some hundreds of MiB, charged to the task's memory quota as its pages are.
   - Or a bound per program, named by its package.
   - The kernel track chooses which.
   - Whether instances of one program share its read-only pages matters here too: a browser runs one content process per site.
2. **When 264 or a native inference runtime starts:**
   - `PT_TLS`, and threads in one address space with a wait on a memory word (262-KRN-0163 kept them "possible later, for a POSIX layer");
   - an application starting the helper programs its own package names.

