# issues/ — working tasks

## Rules

1. **`issues/` holds only the tasks that are being worked on or scheduled for the near term.** Long-term direction lives in [ROADMAP.md](../ROADMAP.md); a roadmap item becomes an issue here when work on it is about to start.
2. **A finished task leaves this directory.** When its acceptance criteria are met, move it with `git mv` to [`issues-done/`](../issues-done/) and change the extension to `.done`, in the same commit as the work (or right after it). In the moved file:
   - append ` — done` to the title and set `Status: done (YYYY-MM-DD)`;
   - add a `## Resolution` (or `## Done`) section saying what was done and where (files, tests);
   - fix relative links (`../issues/…md` for open tasks, `….done` for finished ones).
3. **A task that became irrelevant** (replaced by another design or by a different task) is moved the same way with status `superseded` and a resolution naming what replaced it. Unfinished remainders are split into a new issue rather than keeping the old one open.
4. **One file per task**: `NNN-TRK-MMMM-short-name.md` — `NNN` the main task it belongs to (`000`: none), `TRK` the track's code, `MMMM` the track's own counter — or `NNN-short-name.md` for a main task (numbered from the track's range). Numbers are never reused; numbers given before this scheme (`158`, `u015`, …) stay. Format: title, metadata line (Type · Priority · Status · Blocked by), Problem, Plan, Acceptance criteria, Related. Every task names the Constitution articles or roadmap item it serves.
5. Update the tables below in the same commit.

Tasks that need a person (repository settings, legal decisions, coordination of agent sessions) are in [issues-human/](../issues-human/README.md).

## Open tasks

Tracks work in parallel; open tracks can be taken now. Their codes, ranges, owners, branches and starting tasks are in the registry [TRACKS.md](../TRACKS.md). A task is numbered `NNN-TRK-MMMM` (main task, track code, the track's own counter); a request to another track goes to `requests-<TRK>.md`; ABI changes are made only in `KRN` tasks.

| № | Task | Type / owner | Priority | Blocked by | Roadmap |
|---|---|---|---|---|---|
| [158](158-video-capture.md) | Video capture devices: the video gateway with consent, the camera mark and `camera` are done on a synthetic source; UVC cameras over `usb_host` (isochronous transfers) open | kernel + services | P2 | — | tracks A, G |
| [u015](u015-pins.md) | `pins`: the pins of an ARM board — list, every function of a pin with the active one marked, levels and changes through `gpio` (done except the board run) | tools | P2 | 205 | track H |
| [u017](u017-pins-view.md) | `pinmap`: the board's header on a screen, changes by keys after one confirmation; `pins` and `pinmap` from `wm` and `console` (done except the board run) | tools | P3 | 205 | track H |
| [205](205-aarch64-boards.md) | aarch64 on boards with UEFI: Raspberry Pi 4/5 (EDK2), servers with ACPI | porting | P2 | — (201–204 done) | track H |
| [207](207-gpio-service.md) | `gpio`: a user-space service for the pins of ARM boards (BCM2711, PL061); hwdocs pin tables | porting (done except hardware) | P2 | — (206 done) | track H |
| [210](210-apple-silicon-native.md) | Apple Silicon Macs natively (M1 first): boot through m1n1 and U-Boot, device tree, AIC, spin table, DART, DWC3 USB | main task, `PRT` | P3 | a Mac with M1; 205 | track H |
| [300](300-checksummed-block-store.md) | A checksummed block store with content addresses (track B, first step): the identifier format is done (`300-STO-0001`), the service is built (`300-STO-0002`) | main task, `STO` | P2 | — | track B |
| [302](302-names-and-current-roots.md) | Names and their current roots (track B, third step): versions published by compare-and-swap, only complete roots (`302-STO-0001` done in the store) | main task, `STO` | P2 | — | track B |
| [301](301-objects-as-merkle-dags.md) | Objects larger than a block as a Merkle-DAG of chunks and DAG-CBOR nodes (track B, second step): the format, the library and the store's nodes are done (`301-STO-0001`, `0002`); objects over the running service wait for it | main task, `STO` | P2 | — | track B |
| [300-STO-0004](300-STO-0004-rights-by-badge.md) | Rights to the block store by badge: `BADGE_GET` reads, `BADGE_PUT` stores, refusals logged; host-tested, waits for badged clients | `STO` | P2 | [300-KRN-0001](../issues-done/300-KRN-0001-blockstore-at-boot.done) (done) | track B |
| [300-STO-0002](300-STO-0002-blockstore-service.md) | The `blockstore` service: put and get by CID, append-only, every read checked; built and host-tested, waits to be started at boot | `STO` | P2 | [300-KRN-0001](../issues-done/300-KRN-0001-blockstore-at-boot.done) (done) | track B |
| [350](350-signed-boot-images.md) | Signed boot images and a launch record (track C, first step) | main task, `UPD` (open) | P2 | — | track C |
| [400](400-marain-m0-m2-host-bench.md) | Marain M0–M2 on a host bench (track E, first step) | main task, `MRN` (open) | P3 | — | track E |
| [500](500-fuzzing-abi-and-idl.md) | Fuzzing the system calls and the IDL decoders (Assurance, first step) | main task, `ASR` (open) | P2 | — | Assurance |
| [158-APP-0005](158-APP-0005-camera-mark-on-ci.md) | The camera mark was missing once on CI (NVMe group); the check now reports what follows such a failure | `APP` | P2 | the next failure | tracks A, G |
| [171-APP-0006](171-APP-0006-monitor-bounds.md) | `top`, `memmap`, `load`: capabilities as `n/4095`, task and endpoint counts without the root quota's 65535, the task graph to its own scale (requested by `KRN`) | `APP` | P2 | — | G |
| [171-APP-0007](171-APP-0007-sysinfo-every-cpu-and-capability.md) | `sysinfo`: every CPU (up to 255) and every capability of a task, in `top`, `load` and `caps` (requested by `KRN`) | `APP` | P2 | — | G |

Requests that wait for a track to number them: [requests-KRN.md](requests-KRN.md) (kernel structures outside the 64 MiB arena; the busy suite's share check and a busy host).


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
| [088](../issues-done/088-text-window-manager.done) | `wm`: window manager — text and pixel programs in windows (keys and mouse, snapping, detach keeps them running); programs open windows through `mind::windowed` | done (2026-10-05) |
| [089](../issues-done/089-text-clock-faces.done) | Text faces for `clock` and `dzen-clock` (`--text`: large digits, colored cells; on a screen or in a `wm` text window) | done (2026-10-05) |
| [090](../issues-done/090-read-only-status-and-modifier-key-bars.done) | Editor says READ-ONLY; key bars follow Shift, Ctrl and Alt | done (2026-10-04) |
| [091](../issues-done/091-program-list-fits-the-screen.done) | `list`: sorted in columns that fit the screen; `list -l` says what each program does | done (2026-10-04) |
| [092](../issues-done/092-help-for-every-program.done) | `help <program>`, and `--help` in every application (`mind::about!`) | done (2026-10-04) |
| [093](../issues-done/093-screen-recording.done) | `record`: the screen as AVI/MJPEG (`mind::jpeg` with a restart per row of blocks, `mind::avi`); one window: u014 | done (2026-10-06) |
| [094](../issues-done/094-shell-script-language.done) | `msh`: the shell's script language — values, records, results as in Marain (`?`, `or`, `try`), commands as typed, `capture`/`ps()`/`files()`, `requires:` limits a script's authority | done (2026-10-06) |
| [095](../issues-done/095-list-by-mask.done) | `list a*`: the programs whose names match a mask (`mind::mask` without allocation) | done (2026-10-04) |
| [096](../issues-done/096-audio-without-interrupts.done) | `say` and `listen` hung when the sound card shared its interrupt line (`audio_gw` looks at its ring while a client waits) | done (2026-10-05) |
| [097](../issues-done/097-fm-command-line-and-hidden-panels.done) | fm: the command line under the panels (`cd`, `edit`, `view`, programs with arguments) and hiding panels with Ctrl+O, Ctrl+F1/F2, Ctrl+P | done (2026-10-05) |
| [098](../issues-done/098-pong-shows-the-exchange.done) | `pong` kept the string it read on screen for 10 ms only; it stays now, with the number of calls, and Esc works between calls | done (2026-10-05) |
| [099](../issues-done/099-fm-starts-programs-in-windows.done) | fm in a window of `wm` starts programs in windows of their own (it lends its broker client, files and system information) | done (2026-10-05) |
| [u001](../issues-done/u001-mouse-in-windows-and-fm.done) | The mouse inside windows (`wm` passes clicks, drags and the wheel to the program at the cell of its content) and in fm (click, double click, right click, wheel, key bar; the pointer cell on a screen) | done (2026-10-05) |
| [u002](../issues-done/u002-restore-and-unsnap-windows.done) | wm: `[▲]` maximizes, `[⇕]` gives a maximized or snapped window its frame back; dragging a snapped title off the edge does too | done (2026-10-05) |
| [u003](../issues-done/u003-desktop-programs-menu.done) | wm: a right click on the desktop (or Alt+P) opens the programs by category; a click starts one in a window | done (2026-10-05) |
| [u004](../issues-done/u004-console.done) | `console`: a terminal for programs in a window or on a screen; console programs started in `wm` or `fm` run in it | done (2026-10-05) |
| [u005](../issues-done/u005-beep-without-a-screen.done) | `beep` without a screen: `beep 440` sounds 500 ms, `beep 440 200 0 100 880 300` a series (0 Hz: a pause) | done (2026-10-05) |
| [u006](../issues-done/u006-console-commands.done) | console: its own `ps`, `ls`, `cat`, `date`, `time`, `ping` (through its policy grant); the shell's commands named; `run <program>` | done (2026-10-05) |
| [u007](../issues-done/u007-time-clock-dzen-text.done) | The shell's one-line `clock` is `time`, `clock` starts the clock again; dzen-clock: T switches to the text face | done (2026-10-05) |
| [u008](../issues-done/u008-clickable-top-bar.done) | wm: the items of the top bar can be clicked instead of their keys (for a host that keeps Alt+Tab for itself) | done (2026-10-05) |
| [u009](../issues-done/u009-pixel-windows-follow-their-frame.done) | wm: a pixel window's content follows its frame (`clock` and `dzen-clock` laid out again at its size) | done (2026-10-05) |
| [u010](../issues-done/u010-say-text-on-screen.done) | `say` shows its text whole on its screen, Cyrillic as it is (8x16 font, rows cut at spaces) | done (2026-10-05) |
| [u011](../issues-done/u011-beep-from-the-desktop-menu.done) | beep from wm's desktop menu: works (its lines in console, its tones in a WAV), now tested; console says when every program ends | done (2026-10-06) |
| [u012](../issues-done/u012-load-graphs-aligned.done) | `load`: every graph ends at the same column and the scale labels end in one column, whatever their widths | done (2026-10-06) |
| [u013](../issues-done/u013-quit-from-menus-and-key-bars.done) | Quit in a program's menu and key bar: `edit` and `view` take the mouse, menus take clicks, F10 quits from edit's open menu | done (2026-10-06) |
| [u014](../issues-done/u014-record-a-window.done) | `record -w` in wm's run line records the window in front alone: wm lends a read-only lease of its surface and marks its frame " ● REC " | done (2026-10-06) |
| [u016](../issues-done/u016-clock-console-faces.done) | `clock --line`, `dzen-clock --line`: console programs whose line is written again with `\r`; `REQUEST_LINE` and `mind::process::console_run` | done (2026-10-06) |
| [170-APP-0001](../issues-done/170-APP-0001-escrow-in-caps.done) | `caps` and `top` name the escrow capability kind (issue 170): `escrow ----  of control` | done (2026-10-06) |
| [000-APP-0004](../issues-done/000-APP-0004-pinmap-check-whole-line.done) | The `pinmap` check waits for the whole `[PINMAP] READY` line (the storage track's request) | done (2026-10-06) |
| [000-APP-0003](../issues-done/000-APP-0003-svc-restart-loader-race.done) | `svc restart loader` could lose its own start: svc first makes one call to the loader, which then has answered the shell | done (2026-10-06) |
| [171-APP-0002](../issues-done/171-APP-0002-sysinfo-pages.done) | `sysinfo.wit` 4.0: tasks and endpoints page by page; `sysmon`, `top`, the console's `ps`, `logd` see every task (171) | done (2026-10-06) |
| [100](../issues-done/100-virtio-net-driver.done) | `virtio_net`: network card driver in ring 3 | done (2026-10-04) |
| [101](../issues-done/101-network-stack.done) | Network stack `netstack` (DHCP, ICMP, DNS, UDP, TCP) | done (2026-10-04) |
| [104](../issues-done/104-virtio-modern-msix.done) | Modern VirtIO interface and MSI-X interrupts | done (2026-10-04) |
| [102](../issues-done/102-network-policy-broker.done) | Network policy broker and flow grants | done (2026-10-04) |
| [103](../issues-done/103-tls-service.done) | TLS service with non-exportable keys | done (2026-10-04) |
| [105](../issues-done/105-multiple-network-cards.done) | Several network cards: driver instances per card, stack interfaces | done (2026-10-04) |
| [106](../issues-done/106-network-offloads.done) | Checksum and segmentation offloads, after measurement (transmit checksum offload, off by default) | done (2026-10-04) |
| [107](../issues-done/107-batched-frame-path.done) | Batched frame path between the stack and the card drivers (frame ring) | done (2026-10-04) |
| [150](../issues-done/150-user-memory-beyond-the-arena.done) | User memory beyond the kernel arena: frame pool, memory quotas delegated at `SPAWN`, large sealed objects | done (2026-10-05) |
| [153](../issues-done/153-xsave-avx-state.done) | XSAVE: AVX state per task | done (2026-10-05) |
| [154](../issues-done/154-push-to-talk-routing.done) | Push-to-talk routing: keys taken from the focused program for a listener (`INPUT_LISTEN`) | done (2026-10-05) |
| [156](../issues-done/156-ps2-mouse.done) | PS/2 mouse: pointer events for the focused program | done (2026-10-04) |
| [157](../issues-done/157-window-broker.done) | `windows`: window broker, windows that outlive the window manager | done (2026-10-04) |
| [159](../issues-done/159-shared-interrupt-lines.done) | Shared interrupt lines reach every driver on them | done (2026-10-05) |
| [160](../issues-done/160-absolute-pointer-tablet.done) | Absolute pointer: a VirtIO tablet, no pointer grab in the emulator (the porting stream's; the same work as 161, merged into it; 160-focus-for-a-started-program is another issue) | merged into 161 (2026-10-05) |
| [200](../issues-done/200-architecture-layer.done) | Architecture layer in the kernel and libmind (x86-64 first) | done (2026-10-05) |
| [201](../issues-done/201-aarch64-boot.done) | aarch64 on QEMU `virt`: boot to init | done (2026-10-05) |
| [202](../issues-done/202-aarch64-devices.done) | aarch64 devices: PCIe ECAM and the ITS, VirtIO block/net/input, PL011, PL031, display | done (2026-10-05) |
| [203](../issues-done/203-aarch64-smp-and-power.done) | aarch64 SMP: CPUs from the MADT started through PSCI, SGIs, reset and power off | done (2026-10-05) |
| [204](../issues-done/204-aarch64-profile-and-ci.done) | aarch64 profile `aarch64/QEMU-virt-0` and CI: `ARCH=aarch64 ./02_build.sh`, three CI groups | done (2026-10-05) |
| [206](../issues-done/206-pin-controllers-from-firmware.done) | aarch64: pin controllers (BCM2711 GPIO, PL061) from the DSDT and SSDTs, their registers by `PLATFORM_MMIO` index | done (2026-10-06) |
| [208](../issues-done/208-aarch64-idle-check-margin.done) | aarch64 idle check: the lowest of three samples against 0.85 s; passes under load, fails when idle CPUs spin (3.85 s) | done (2026-10-06) |
| [151](../issues-done/151-shell-grant-slots-13-15.done) | Shell grant slots 13–15: authority view, keyboard, display | done (2026-10-04) |
| [152](../issues-done/152-reboot-system-call.done) | `REBOOT` system call | done (2026-10-04) |
| [161](../issues-done/161-absolute-pointer-virtio-tablet.done) | Absolute pointer events and the `virtio_input` driver: with QEMU's VirtIO tablet the system's pointer follows the host's to every edge | done (2026-10-05) |
| [162](../issues-done/162-console-output-slot.done) | `SLOT_CONSOLE`: a launcher may lend an endpoint where what the program prints goes too (`mind::output`) | done (2026-10-05) |
| [163](../issues-done/163-window-broker-memory.done) | init gives the window broker a 128 MiB memory quota: pixel windows with room for the screen | done (2026-10-05) |
| [165](../issues-done/165-display-client-for-programs.done) | A launcher may lend the compositor client (`REQUEST_DISPLAY`); the compositor shows a red dot while the screen is captured | done (2026-10-06) |
| [164](../issues-done/164-usb-hid-keyboard-and-mouse.done) | USB keyboards, mice and tablets: `usb_host` (xHCI, hubs, hot plug, `idl/usb.wit`), `usb_hid`, `usb_storage` over it; no `ps2_kbd` without a controller; Intel chipsets' ports moved from EHCI | done (2026-10-06) |
| [155](../issues-done/155-virtual-consoles.done) | Virtual consoles: the shell's four consoles, Ctrl+Alt+F1…F4 whatever program has the keyboard, each with its own text, history and programs | done (2026-10-06) |
| [166](../issues-done/166-exit-status-for-launchers.done) | `EXIT` with a code, `EXIT_STATUS` (58) for the last 16 tasks that ended; `grep` exits 0/1/2; `msh` makes a failed program `err` | done (2026-10-06) |
| [160](../issues-done/160-focus-for-a-started-program.done) | The task in front hands the focus to a program it starts (`SPAWN_FOREGROUND`, `loader.wit` 1.5 `commit-in-front`) and gets it back when it ends; `fm` and `console` on a full screen use it | done (2026-10-06) |
| [167](../issues-done/167-models-of-revoke-and-move.done) | TLA+ models of revoke and MOVE checked by TLC (stage II exit); a mapped memory capability no longer moves (the bug the model found) | done (2026-10-06) |
| [168](../issues-done/168-task-memory-charged-to-spawner.done) | A task's image, stack and screen are charged to its spawner's memory quota (MC-1.7) | done (2026-10-06) |
| [169](../issues-done/169-recovery-reserve.done) | A recovery reserve of the frame pool (32 MiB, set by init): applications cannot take the memory a service restart needs (MC-6.5) | done (2026-10-06) |
| [209](../issues-done/209-aarch64-smoke-interleaved-lines.done) | aarch64 smoke: a line goes out in one write, so lines of services printing at once stay whole (reported by the tools track) | done (2026-10-06) |
| [170](../issues-done/170-supervisor-without-usable-privileges.done) | The supervisor without usable privileges: init keeps the privileges it grants in escrow and holds no process control (MC-3.12, C6) | done (2026-10-06) |
| [171](../issues-done/171-limits-from-the-hardware.done) | Limits from the hardware: all RAM, every CPU, growing task, endpoint and capability tables, STAT pages (`KRN` tasks 0001–0005, 0007); `sysinfo` requested from `APP` | done (2026-10-06) |
| [171-KRN-0001](../issues-done/171-KRN-0001-ram-above-4g.done) | x86-64: task memory from all RAM, above 4 GiB too (171, step 1) | done (2026-10-06) |
| [171-KRN-0005](../issues-done/171-KRN-0005-frame-pool-ranges.done) | The frame pool takes every free range of the firmware map (171, step 5) | done (2026-10-06) |
| [171-KRN-0003](../issues-done/171-KRN-0003-every-cpu.done) | Every CPU the firmware reports (x86 up to xAPIC's 255); no tick for idle CPUs (171, step 3) | done (2026-10-06) |
| [171-KRN-0004](../issues-done/171-KRN-0004-growing-capability-tables.done) | Capability tables that grow on demand up to 4095 slots (171, step 4) | done (2026-10-06) |
| [171-KRN-0002](../issues-done/171-KRN-0002-task-and-endpoint-tables.done) | No fixed count of tasks, applications or endpoints; root quota 65 535 (171, step 2) | done (2026-10-06) |
| [171-KRN-0007](../issues-done/171-KRN-0007-stat-pages.done) | STAT from a given record on: callers page through any number of records (171) | done (2026-10-06) |
| [300-KRN-0001](../issues-done/300-KRN-0001-blockstore-at-boot.done) | The block store starts at boot over `ramdisk#1`; the shell's client in slot 25; `REQUEST_BLOCKSTORE` (requested by STO) | done (2026-10-06) |
| [300-STO-0001](../issues-done/300-STO-0001-content-identifiers.done) | Content identifiers: CIDv1 (`raw`, SHA-256) and SHA-256 in `libmind`, unsupported and non-canonical forms refused (MC-4.2, 4.13) | done (2026-10-06) |
| [301-STO-0001](../issues-done/301-STO-0001-object-format.done) | The object format: 16 KiB chunks and DAG-CBOR nodes with a shape fixed by the size, a builder and a checking reader (`mind::dag`) | done (2026-10-06) |
| [301-STO-0002](../issues-done/301-STO-0002-store-takes-nodes.done) | The block store takes nodes: `put(codec, data)`, a `dag-cbor` block stored only if it is a canonical node of `mind::dag` | done (2026-10-06) |
| [302-STO-0001](../issues-done/302-STO-0001-names-in-the-store.done) | Names in the block store: `publish` by compare-and-swap on the version, only roots whose object is complete; `resolve`; `BADGE_PUBLISH` | done (2026-10-06) |

Issues 052–071 implement the [system tools plan](../docs/tools/README.md); they were numbered 032–051 on the tools branch and renumbered by [051](../issues-done/051-merge-main-into-tools.done) (each record says "Formerly tools-branch NNN."). Issues 040–043 and 045–050 were the plan's open specs on `main`; the tools records replaced them. Issues 001–011 were opened after the review of 2026-09-17 (handoff ↔ code, see [knowledge/04](../knowledge/04-handoff-vs-code-matrix.md)).
