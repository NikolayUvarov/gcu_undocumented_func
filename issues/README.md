# issues/ — working tasks

## Rules

1. **`issues/` holds only the tasks that are being worked on or scheduled for the near term.** Long-term direction lives in [ROADMAP.md](../ROADMAP.md); a roadmap item becomes an issue here when work on it is about to start.
2. **A finished task leaves this directory.** When its acceptance criteria are met, move it with `git mv` to [`issues-done/`](../issues-done/) and change the extension to `.done`, in the same commit as the work (or right after it). In the moved file:
   - append ` — done` to the title and set `Status: done (YYYY-MM-DD)`;
   - add a `## Resolution` (or `## Done`) section saying what was done and where (files, tests);
   - fix relative links (`../issues/…md` for open tasks, `….done` for finished ones).
3. **A task that became irrelevant** (replaced by another design or by a different task) is moved the same way with status `superseded` and a resolution naming what replaced it. Unfinished remainders are split into a new issue rather than keeping the old one open.
4. **One file per task**: `NNN-short-name.md`, numbers are never reused. Format: title, metadata line (Type · Priority · Status · Blocked by), Problem, Plan, Acceptance criteria, Related. Every task names the Constitution articles or roadmap item it serves.
5. Update the tables below in the same commit.

Tasks that need a person (repository settings, legal decisions, coordination of agent sessions) are in [issues-human/](../issues-human/README.md).

## Open tasks

Tracks work in parallel and number their issues from separate ranges so the numbers never collide: the **tools track** (077–099) builds the user tools after its plan ([docs/tools](../docs/tools/README.md), [docs/voice](../docs/voice/README.md); branch `claude/wizardly-franklin-kec1a9`); the **network track** (100–149) builds the network drivers and services of track D; the **kernel track** (150–199) owns `kernel/`, `common/abi.rs` and the core services. A tools issue blocked by a kernel issue waits for it. ABI changes are made only in kernel issues.

| № | Task | Type / owner | Priority | Blocked by | Roadmap |
|---|---|---|---|---|---|
| [088](088-text-window-manager.md) | `wm`: text window manager (windows, dragging, snapping to edges and corners, programs in windows) | tools | P2 | — (mouse: 156) | track G |
| [150](150-user-memory-beyond-the-arena.md) | User memory beyond the kernel arena (frames from free RAM, large shared read-only objects) | kernel | P2 | — | stage II, track G |
| [153](153-xsave-avx-state.md) | XSAVE: AVX state per task | kernel | P3 | — | track G |
| [154](154-push-to-talk-routing.md) | Push-to-talk routing to a registered listener | kernel | P3 | — | track G |
| [155](155-virtual-consoles.md) | Virtual consoles: several shell consoles, Alt+F1…F4 | shell + kernel | P2 | — (extends 154) | track G |
| [156](156-ps2-mouse.md) | PS/2 mouse: pointer events for the focused program | kernel | P2 | — | track G (for 088) |

## Finished tasks (`issues-done/`)

| № | Task | Result |
|---|---|---|
| [001](../issues-done/001-flat-binary-entry-offset-and-got-call.done) | Flat binaries: `_start` offset and GOT call | superseded by 002 (2026-10-03) |
| [002](../issues-done/002-elf-loader.done) | ELF loader instead of flat binaries | done (2026-10-03) |
| [003](../issues-done/003-bss-and-heap-allocator.done) | `.bss`, statics, heap | done (2026-10-03) |
| [004](../issues-done/004-apic-idt-interrupts.done) | IDT + APIC, interrupt-driven timer and keyboard | done (2026-10-03) |
| [005](../issues-done/005-syscalls.done) | `int 0x80` system calls, shared ABI | done (2026-10-03) |
| [006](../issues-done/006-bootloader-load-from-fat32.done) | Bootloader reads images from FAT | done (2026-10-03) |
| [007](../issues-done/007-kernel-font-and-primitives.done) | Font, primitives, console | done; panic diagnostics split into 018 (2026-10-03) |
| [008](../issues-done/008-kernel-timeout-handoff.done) | Handoff to userspace on timeout | superseded (2026-10-03) |
| [009](../issues-done/009-gop-pixel-format.done) | Honour `PixelFormat` and select a GOP mode | done (2026-10-04) |
| [010](../issues-done/010-docs-sync.done) | Sync README/handoff with the code | done (2026-10-03) |
| [011](../issues-done/011-reproducible-toolchain.done) | Reproducible build: pinned toolchain, lock files, CI | done (2026-10-04) |
| [012](../issues-done/012-multitasking-and-program-instances.done) | Multitasking, instances, `ps`/`kill`/`fg` | done (2026-09-18) |
| [013](../issues-done/013-smp-and-memory-isolation.done) | SMP, ring 3, memory isolation | done (2026-09-19) |
| [014](../issues-done/014-private-program-heap.done) | Private program heap | done (2026-09-19) |
| [015](../issues-done/015-load-programs-through-vfs.done) | Loading programs through the VFS | done (2026-10-03) |
| [016](../issues-done/016-storage-drivers.done) | ATA / AHCI / USB storage drivers | done (2026-10-03) |
| [017](../issues-done/017-tts-on-audio-gateway.done) | Text to speech over the audio gateway | done (2026-10-03) |
| [018](../issues-done/018-kernel-panic-diagnostics.done) | Kernel panic prints message, location, CPU and task | done (2026-10-04) |
| [019](../issues-done/019-platform-profile-x86-64-qemu-0.done) | Platform profile `x86-64/QEMU-0` | done (2026-10-03) |
| [020](../issues-done/020-init-bootstrap-authority.done) | `init`: bootstrap authority, driver policy out of the kernel | done (2026-10-03) |
| [021](../issues-done/021-shell-in-ring-3.done) | Shell in ring 3, input/focus policy out of the kernel | done (2026-10-03) |
| [022](../issues-done/022-audio-tools-say-listen.done) | Audio tools `say <text>`, `listen`; program arguments; AC97 capture | done (2026-10-03) |
| [023](../issues-done/023-monotonic-clock.done) | Monotonic clock with defined resolution | done (2026-10-03) |
| [024](../issues-done/024-accounted-kernel-objects.done) | Per-owner quotas for tasks and endpoints | done (2026-10-03) |
| [025](../issues-done/025-capability-generations.done) | Capability handles with generations | done (2026-10-03) |
| [026](../issues-done/026-derivation-and-revocation.done) | Capability derivation, attenuation and revocation | done (2026-10-03) |
| [027](../issues-done/027-no-ambient-endpoint-names.done) | No ambient endpoint names | done (2026-10-03) |
| [028](../issues-done/028-memory-rights-and-lease.done) | Memory rights, read-only mappings, leases ended by revoke | done (2026-10-03) |
| [029](../issues-done/029-memory-objects-move-seal.done) | Memory objects: MOVE and sealed SHARE_RO | done (2026-10-04) |
| [030](../issues-done/030-ipc-bounds-and-cancellation.done) | IPC bounds, timeouts and cancellation | done (2026-10-04) |
| [031](../issues-done/031-mind-idl-v0.done) | MIND IDL v0: WIT subset, generated bindings, receiver checks (`rtc`) | done (2026-10-04) |
| [032](../issues-done/032-minimal-supervision.done) | Minimal supervision: exit notices, restart budget, quarantine, fencing | done (2026-10-04) |
| [033](../issues-done/033-audit-and-supervision-follow-ups.done) | Follow-ups: multi-process tests, device stop, platform privilege dropped, quotas | done (2026-10-04) |
| [034](../issues-done/034-key-events-input-queue.done) | Key events in the per-task input queue | done (2026-10-04) |
| [035](../issues-done/035-observe-and-stat.done) | OBSERVE privilege, `STAT`, firmware memory map | done (2026-10-04) |
| [036](../issues-done/036-endpoint-badges.done) | Endpoint badges | done (2026-10-04) |
| [037](../issues-done/037-task-limit.done) | Task limit 32, 127 endpoints | done (2026-10-04) |
| [038](../issues-done/038-scheduling-budgets.done) | Scheduling budgets and bands (C7) | done (2026-10-04) |
| [039](../issues-done/039-port-services.done) | Port the services to MIND IDL and the C4 memory modes (C8) | done (2026-10-04) |
| [040](../issues-done/040-program-heap.done) | Program heap `mind::alloc` | superseded by 052 (2026-10-04) |
| [041](../issues-done/041-font-8x16.done) | 8×16 font with Cyrillic and box drawing | superseded by 053 (2026-10-04) |
| [042](../issues-done/042-tui-library.done) | TUI library `mind::tui` | superseded by 054 (2026-10-04) |
| [043](../issues-done/043-keyboard-decoding-and-line-editing.done) | Keyboard decoding, layouts, VT100 input, shell line editing | superseded by 055, 056 (2026-10-04) |
| [044](../issues-done/044-idl-v02-records-strings.done) | MIND IDL v0.2: records, strings, lists in buffers; `loader.wit` | done (2026-10-04) |
| [045](../issues-done/045-sysmon-and-monitors.done) | `sysmon` and `top`, `memmap`, `load`, `hw` | superseded by 060, 061 (2026-10-04) |
| [046](../issues-done/046-loader-sessions.done) | Loader v1: launch sessions with granted capabilities | superseded by 062 (2026-10-04) |
| [047](../issues-done/047-viewer-and-fm-readonly.done) | Viewer `view`, file manager `fm` read-only | superseded by 057, 063 (2026-10-04) |
| [048](../issues-done/048-write-path-ramdisk-vfs2.done) | Block write, `ramdisk`, VFS v2 with directory handles | superseded by 064–066 (2026-10-04) |
| [049](../issues-done/049-editor-and-fm-write.done) | Editor `edit`, `fm` write operations, `df`, `fsck` | superseded by 067, 068 (2026-10-04) |
| [050](../issues-done/050-logd-dmesg-svc.done) | `logd`, `dmesg`, `svc` | superseded by 069, 070 (2026-10-04) |
| [051](../issues-done/051-merge-main-into-tools.done) | Merge `main` into the tools branch, reconcile duplicate IDL/STAT/input/badges and issue numbers | done (2026-10-04) |
| [052](../issues-done/052-program-heap.done) | Program heap `mind::alloc` | done (2026-10-04) |
| [053](../issues-done/053-font-8x16.done) | 8×16 font with Cyrillic and box drawing | done (2026-10-04) |
| [054](../issues-done/054-tui-library.done) | Text UI library `mind::tui` | done (2026-10-04) |
| [055](../issues-done/055-key-events.done) | Key events: E0 keys, modifiers, layouts, VT100 | done (2026-10-04); on the event words of 034 after the merge |
| [056](../issues-done/056-shell-line-editing.done) | Shell: line editing, history, Cyrillic | done (2026-10-04) |
| [057](../issues-done/057-viewer.done) | Viewer `view` | done (2026-10-04) |
| [058](../issues-done/058-mind-idl-v0.2.done) | MIND IDL v0.2: records, strings, lists | done (2026-10-04); after the merge 044 is the base, enums, `bytes<N>` and capability results are its minor extension |
| [059](../issues-done/059-observation-abi.done) | OBSERVE privilege, `STAT`, firmware memory map | done (2026-10-04); after the merge the `STAT` of 035 is used, lost fields in 075 |
| [060](../issues-done/060-sysmon.done) | `sysmon` service | done (2026-10-04) |
| [061](../issues-done/061-top-memmap-load-hw.done) | `top`, `memmap`, `load`, `hw` | `monitor/`: `top`, `memmap`, `load`, `hw` on sysmon; `Key::latin` |
| [062](../issues-done/062-loader-v1-launch-grants.done) | Loader v1: launch with granted capabilities | launch sessions (`idl/loader.wit` 1.1), `mind::request!`, console programs, `uptime` program |
| [063](../issues-done/063-file-manager-read-only.done) | File manager `fm`, read-only | `fm`: two panels, viewer, quick view, info, find, run; VFS LIST with attributes and times |
| [064](../issues-done/064-endpoint-badges-block-write.done) | Endpoint badges and block write | BLOCK_WRITE/FLUSH for the write badge (`idl/block.wit` 1.1 after the merge, badges of 036); ATA/AHCI/USB write; `block` suite |
| [065](../issues-done/065-ramdisk.done) | `ramdisk` block service | 8 MiB RAM disk service, formatted FAT16 and mounted as ram: |
| [066](../issues-done/066-vfs-v2-fat-write.done) | `vfs_server` v2: directory handles, FAT write | vfs.wit 2.x with handles and zones; FAT12/16/32 writer with long names; write-back cache; shell file commands |
| [067](../issues-done/067-editor.done) | Editor `edit` | edit: piece table with undo, search/replace, menu and dialogs; saves via name.tmp; REQUEST_FILE lends the user's VFS client |
| [068](../issues-done/068-fm-write-df-fsck.done) | `fm` write operations, `df`, `fsck` | fm: copy/move/mkdir/delete jobs on A: and ram:, built-in editor; df; fsck (vfs.wit 2.1 check) |
| [069](../issues-done/069-logd-dmesg.done) | `logd` and `dmesg` | logd: ring with stamped sources, rate limit, read badge; println lines of services go there; dmesg; logger |
| [070](../issues-done/070-svc-lifecycle.done) | `svc` and lifecycle control | lifecycle requests served by init (`idl/init.wit` 1.1 after the merge); svc; top stops and restarts |
| [071](../issues-done/071-scoped-file-grants.done) | Scoped file grants for launched programs | vfs.wit scope: the editor's client is confined to its file's directory and revoked on exit; REQUEST_FILES for fm |
| [072](../issues-done/072-fixed-grant-slots.done) | Fixed capability slots for launcher grants (`SLOT_DYNAMIC` 16) | done (2026-10-04) |
| [073](../issues-done/073-port-out-block.done) | `PORT_OUT_BLOCK`: block writes of 16-bit words to a port | done (2026-10-04) |
| [074](../issues-done/074-exited-console-output.done) | Output of an exited console program stays readable | done (2026-10-04) |
| [075](../issues-done/075-stat-fields-for-the-monitors.done) | `STAT` fields the monitors lost in the merge (`STAT_VERSION` 2) | done (2026-10-04) |
| [076](../issues-done/076-monitors-show-restored-stat-fields.done) | The monitors show the restored `STAT` fields (`sysinfo.wit` 2.0) | done (2026-10-04) |
| [077](../issues-done/077-voice-audio-front-end.done) | Voice V0: audio front end (`mind::voice`, `listen --vad/--wav`) | done (2026-10-04) |
| [078](../issues-done/078-voice-command-recognizer.done) | Voice V1: offline command recognizer (`hear`, `mind::voice` model and grammar) | done (2026-10-04) |
| [079](../issues-done/079-voice-control-in-the-shell.done) | Voice V2: voice control in the shell (`voice`, `idl/voice.wit`, confirmations, spoken replies, `audio.wit` 1.1) | done (2026-10-04) |
| [080](../issues-done/080-ipc-tool.done) | `ipc`: endpoints, holders, wait-for graph | done (2026-10-04) |
| [081](../issues-done/081-caps-tool.done) | `caps`: capabilities and the derivation tree (`sysinfo.wit` 3.0 authority) | done (2026-10-04) |
| [082](../issues-done/082-find-and-grep.done) | `find` and `grep` | done (2026-10-04) |
| [083](../issues-done/083-format.done) | `format` for the RAM disk (`vfs.wit` 2.3) | done (2026-10-04) |
| [084](../issues-done/084-reboot.done) | `reboot [-f]`: flush, services stopped in reverse order, reset | done (2026-10-04) |
| [085](../issues-done/085-keymap.done) | `keymap`: layout and switch key (`keyboard.wit` 1.0) | done (2026-10-04) |
| [086](../issues-done/086-screenshot.done) | `screenshot`: the screen as a BMP (`display.wit` 1.0) | done (2026-10-04) |
| [087](../issues-done/087-tts-idle-tone.done) | `tts`: a quiet tone stayed after every phrase (fixed-point limit cycle; filters cleared after 30 ms without excitation) | done (2026-10-04) |
| [090](../issues-done/090-read-only-status-and-modifier-key-bars.done) | Editor says READ-ONLY; key bars follow Shift, Ctrl and Alt | done (2026-10-04) |
| [091](../issues-done/091-program-list-fits-the-screen.done) | `list`: sorted in columns that fit the screen; `list -l` says what each program does | done (2026-10-04) |
| [092](../issues-done/092-help-for-every-program.done) | `help <program>`, and `--help` in every application (`mind::about!`) | done (2026-10-04) |
| [100](../issues-done/100-virtio-net-driver.done) | `virtio_net`: network card driver in ring 3 | done (2026-10-04) |
| [101](../issues-done/101-network-stack.done) | Network stack `netstack` (DHCP, ICMP, DNS, UDP, TCP) | done (2026-10-04) |
| [104](../issues-done/104-virtio-modern-msix.done) | Modern VirtIO interface and MSI-X interrupts | done (2026-10-04) |
| [102](../issues-done/102-network-policy-broker.done) | Network policy broker and flow grants | done (2026-10-04) |
| [103](../issues-done/103-tls-service.done) | TLS service with non-exportable keys | done (2026-10-04) |
| [105](../issues-done/105-multiple-network-cards.done) | Several network cards: driver instances per card, stack interfaces | done (2026-10-04) |
| [106](../issues-done/106-network-offloads.done) | Checksum and segmentation offloads, after measurement (transmit checksum offload, off by default) | done (2026-10-04) |
| [107](../issues-done/107-batched-frame-path.done) | Batched frame path between the stack and the card drivers (frame ring) | done (2026-10-04) |
| [151](../issues-done/151-shell-grant-slots-13-15.done) | Shell grant slots 13–15: authority view, keyboard, display | done (2026-10-04) |
| [152](../issues-done/152-reboot-system-call.done) | `REBOOT` system call | done (2026-10-04) |

Issues 052–071 implement the [system tools plan](../docs/tools/README.md); they were numbered 032–051 on the tools branch and renumbered by [051](../issues-done/051-merge-main-into-tools.done) (each record says "Formerly tools-branch NNN."). Issues 040–043 and 045–050 were the plan's open specs on `main`; the tools records replaced them. Issues 001–011 were opened after the review of 2026-09-17 (handoff ↔ code, see [knowledge/04](../knowledge/04-handoff-vs-code-matrix.md)).
