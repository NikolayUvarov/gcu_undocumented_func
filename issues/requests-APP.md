# Requests for the tools track (APP), not numbered yet

**Owner:** tools track · **Status:** open (2 requests waiting, 2026-10-30) · **Recorded by:** the kernel track (KRN), 2026-10-07

The tools track numbers its own tasks (`NNN-APP-MMMM`), so requests from other tracks wait here. The tools track turns each into a task and removes it from this file. The file is kept while empty because other issues link to it; a new request goes below this line.

Numbered on the tools branch (2026-10-09): clocks and the RTC (000-APP-0012), `log:` and `efivar` in the tools (211-APP-0013), full screen and the list of windows in `wm` (211-APP-0014), `update` (351-APP-0029), `wifi` (550-APP-0033), the audit's A07 and A08 (175-APP-0035, 175-APP-0036), the marked window (211-APP-0037), the message a program that ends at once leaves in its window (211-APP-0039), the shell's commands in `wm` (211-APP-0040), Russian speech on the MacBook Pro with the kernel track's measured note on its hiss and clicks (252-APP-0041) and `date set` (211-APP-0042). Numbered on 2026-10-10: the camera from `wm` and the shell in a window, as the camera from `wm` and `console` (158-APP-0043), the shell's own window (211-APP-0040) and `console` joined to the shell (211-APP-0044, waiting for `SLOT_SHELL` from `KRN`); the shell's window only when asked (211-APP-0045) and a window back to its content's size (211-APP-0046); later that day the menu's tests category (176-APP-0051), voice control as a service (000-APP-0052), what `hear` heard (000-APP-0053), the help keys in the shell's commands (000-APP-0054), the date and time from `wm`'s Settings (000-APP-0055), every menu entry reacting (000-APP-0056) and the machine's memory in `top` and `free` (211-APP-0057); then `cpus` naming AVX-512 and AMX (174-APP-0058); on 2026-10-12 the battery in `wm` and the shell (211-APP-0070); on 2026-10-14 the two races in the `wm` suite, a title written after adoption and Turn off's Enter (000-APP-0073), redirection and pipes in the shell and `msh` (000-APP-0074, its pipes waiting for 000-KRN-0094), and the shell's first frame in the system log (213-APP-0075); on 2026-10-15 the Wi-Fi country selector (550-APP-0082).

Numbered on 2026-10-20:

- the shell lending a TLS client without the device certificate, for 351-NET-0022: [351-APP-0083](../issues-done/351-APP-0083-tls-without-the-device-certificate.done);
- the shell lending the SSH service's client, for 351-NET-0027: [351-APP-0084](../issues-done/351-APP-0084-the-shell-lends-the-ssh-client.done);
- `ip` showing each card's kind and the default card, for 550-NET-0025: [550-APP-0085](../issues-done/550-APP-0085-ip-shows-each-cards-kind.done);
- the safe start shown by the shell and `wm`, for 000-KRN-0076: [000-APP-0086](../issues-done/000-APP-0086-the-safe-start-shown.done);
- `app`, `app2` and `dzen-clock` cleaned, from the critique of 2026-10-14: [000-APP-0087](../issues-done/000-APP-0087-app-app2-and-dzen-cleaned.done);
- `files` reading the kernel from the slot that booted, for 351-UPD-0016: [351-APP-0088](../issues-done/351-APP-0088-files-reads-the-booted-slots-kernel.done);
- screenshots and recordings for the project's page, for 000-KRN-0093: [000-APP-0089](000-APP-0089-screenshots-and-recordings-for-the-page.md);
- horizontal scrolling from a trackpad, for 211-DRV-0018: [211-APP-0090](../issues-done/211-APP-0090-horizontal-scrolling.done), done 2026-10-28 on the across field of 211-KRN-0150;
- `wm`'s moving background drawn by rows, with 000-KRN-0081: [000-APP-0091](000-APP-0091-wms-background-by-rows.md);
- `svc boot`, `enable`, `disable`, `after`, `reset`, for 173: [173-APP-0092](../issues-done/173-APP-0092-svc-boot.done), done 2026-10-29;
- the shell's `effector` command, for 750: [750-APP-0093](750-APP-0093-the-shells-effector-command.md), waiting for 750-EFF-0003 and 750-EFF-0006;
- display output over several monitors, for 177: [177-APP-0094](177-APP-0094-wm-over-several-monitors.md), waiting for 177's steps 0–1.

Numbered on 2026-10-22: `help <name>` reading the program from the booted slot, for 351-KRN-0151: [351-APP-0103](../issues-done/351-APP-0103-help-reads-the-booted-slots-program.done), done; the `wifi` guide naming the station's own join, for 550-NET-0041 (its request is on the kernel session's branch): [550-APP-0104](../issues-done/550-APP-0104-the-wifi-guide-names-the-stations-own-join.done), done once 550-NET-0041 was in `main`.

Numbered on 2026-10-28: the channel the updater holds in `update status`, for 351-UPD-0025, taken into [351-APP-0029](351-APP-0029-update-in-the-shell.md) (`update.wit` 1.1 `metadata`); `run` of an installed application's path, for 750-KRN-0169: [750-APP-0109](../issues-done/750-APP-0109-run-takes-an-installed-applications-path.done); NRL's native `nnd` integration, for 900-NRL-0017 (900's T11): [262-APP-0112](262-APP-0112-nnd-and-nn-wit-stage-0.md), its design for agreement first.

`wifi` following `idl/wifi.wit` 1.0 (the network track, 2026-10-12) was done in 550-APP-0033, and the note on the station's client in `SLOT_WIFI`, the read-only card client and the badged TLS client (550-KRN-0097, 000-KRN-0109) is taken up by 550-APP-0033, 000-APP-0049 and 351-APP-0083.

Notes read and kept as they are (the other tracks' changes stand, and the tools track words nothing otherwise):

- the shell's text for `loader.wit` 1.7 `unreadable` (211-KRN-0050): `CANNOT READ THE PROGRAM: ITS DISK DOES NOT ANSWER (UNPLUGGED?)`;
- the camera lent without a question (158; the maintainer's rule, 2026-10-09);
- the shell showing what a script prints at once (176-KRN-0087);
- a launch session holding every launch slot (000-KRN-0101);
- `check`, `bench` and `kbench` stopping when their console is gone (176-KRN-0103).

## Wi-Fi country and channel selector

Numbered on 2026-10-15 as [550-APP-0082](550-APP-0082-wifi-country-selector.md). Kept as a heading because other issues link to it.

## NRL native nnd integration for 900 T11

Numbered on 2026-10-28 as [262-APP-0112](262-APP-0112-nnd-and-nn-wit-stage-0.md), its design ([docs/nn/runtime.md](../docs/nn/runtime.md)) for agreement first. Kept as a heading because [900-NRL-0017](900-NRL-0017-native-runtime-and-nnd-integration.md) links to it.

### NRL pinned conformance and design clarification (2026-10-29)

For existing [262-APP-0112](262-APP-0112-nnd-and-nn-wit-stage-0.md), NRL agrees on text object files and f64-only `nrl.execution/0`. [900-NRL-0025](../issues-done/900-NRL-0025-native-conformance-and-app-handoff.done) supplies the [pinned MIT bundle](../nrl/tests/fixtures/native-conformance-v0/bundle.json) and [handoff contract](../nrl/specs/native-conformance-v0.md). Please update your next design edition/implementation to name the canonical-writer subset, preserve exact float/typed field encoding, distinguish existing std host readers from the native parser still to implement, retain the individual 16384-value object cap, test whole-request/cumulative work preflight instead of unreachable midway work exhaustion, preserve successful output until copied, distinguish nonfinite-input refusal from internal numeric poisoning, and translate no-deadline explicitly. Also make the lifetime/reuse of a 16-bit badge explicit with KRN: retained old sessions/jobs/delegated capabilities must not become the authority of a new client after badge reuse; this needs an owner design decision and native lifecycle case. The concrete native tests and unchanged model ID/tolerance are in that contract. 262-KRN-0178 is the boot prerequisite; worker-per-job need not wait for 0179. NRL's own work remains within `nrl/`; APP owns `nnd`, `nn.wit`, native parser/glue and its design.

## The shell drops serial input past 64 events in one pass of its loop (for 000-KRN-0188)

Recorded by the kernel track (KRN), 2026-10-30, for [000-KRN-0188](000-KRN-0188-typed-characters-lost-under-load.md).

- **What happens.** A line written to QEMU's serial line faster than the shell's loop reads it loses everything past the 64th byte of one pass, the newline included.
  - Reproduced on x86 in QEMU (4 CPUs, KVM, `main` with the kernel branch's 6a7ca80d): `echo` and 190 characters written to QEMU's stdin at once. The shell's echo held 64 bytes (`echo ` and 59 characters), and no command ran.
  - Under host load the harness's one byte per 10 ms does the same once the shell is held for about 0.65 s. That is likely the `RAM above 4 GiB` failure of 2026-10-30, where 4 of 64 hex digits were lost (000-KRN-0188).
- **Why, as read from the code.**
  - `shell/src/main.rs`'s loop reads the UART while it has a byte (`while let Some(byte) = … Uart::read`) and decodes into `Events`, a list of 64.
  - `Events::push` drops what does not fit. Its comment assumes the 16550's FIFO bounds a pass at 16 bytes.
  - QEMU refills the FIFO at each read of the receive register, from its own buffer and from the host, so one pass can read far more.
- **What is asked.** Read only as many bytes as the pass's events hold, and leave the rest in the UART: QEMU's flow control, and a FIFO, hold it until the next pass. For example:

  ```rust
  // in the main loop, in place of `while let Some(byte) = shell.term.serial.as_ref().and_then(Uart::read) { … }`
  while events.room() >= 3 {
      let Some(byte) = shell.term.serial.as_ref().and_then(Uart::read) else { break };
      vt.feed(byte, now, &mut |event| events.push(event));
  }
  // in `impl Events`
  fn room(&self) -> usize { self.list.len() - self.len }
  ```

  A byte yields at most two events, and `vt.poll` one more, hence 3.
- **What showed the fix.** In a local tree only, with this change: the same 195-byte line written at once, 10 times, arrived whole every time (`echo` of 195 bytes, then the shell's answer).
- **What then shows it in the suites.** 000-KRN-0188's criterion: a 200-character line typed 50 times under four busy loops on the host arrives whole every time, x86 and aarch64. The kernel track can add that check to the `shell` suite once the change is in.

## An arm for `vfs.wit` 2.6's error `conflict` in the shell (for 311-KRN-0189)

Recorded by the kernel track (KRN), 2026-10-30, for 311-KRN-0189 (STO's `storefs`, 311-STO-0032).

- **What changes.** `vfs.wit` 2.6 appends an error `conflict`: a write or `flush` refused because another writer published the file first. `mind::fs::Error` gets `Conflict` with it. `vfs_server` never answers it; `storefs` will.
- **Why the shell.** `shell/src/files.rs` `text` matches `mind::fs::Error` exhaustively, so the shell will not build with 2.6 until it has an arm.
- **What is asked.** One of these:
  - **now:** a last arm `#[allow(unreachable_patterns)] _ => "REFUSED",`, which builds against `main` as it is and against 2.6;
  - **after 2.6 is in `main`:** `Error::Conflict => "CHANGED BY ANOTHER WRITER; READ IT AGAIN"`, or the shell's own words.
- 311-KRN-0189 lands once the arm is in `main`, or in the same landing round as it.

