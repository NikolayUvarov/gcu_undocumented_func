# Requests for the tools track (APP), not numbered yet

**Owner:** tools track · **Status:** open (NRL's native integration of 2026-10-26 numbered as 262-APP-0112 on 2026-10-28; none other waiting) · **Recorded by:** the kernel track (KRN), 2026-10-07

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
- horizontal scrolling from a trackpad, for 211-DRV-0018: [211-APP-0090](211-APP-0090-horizontal-scrolling.md), waiting for an across field in absolute pointer events ([requests-KRN.md](requests-KRN.md));
- `wm`'s moving background drawn by rows, with 000-KRN-0081: [000-APP-0091](000-APP-0091-wms-background-by-rows.md);
- `svc boot`, `enable`, `disable`, `after`, `reset`, for 173: [173-APP-0092](173-APP-0092-svc-boot.md), waiting for 173-KRN-0035;
- the shell's `effector` command, for 750: [750-APP-0093](750-APP-0093-the-shells-effector-command.md), waiting for 750-EFF-0003 and 750-EFF-0006;
- display output over several monitors, for 177: [177-APP-0094](177-APP-0094-wm-over-several-monitors.md), waiting for 177's steps 0–1.

Numbered on 2026-10-22: `help <name>` reading the program from the booted slot, for 351-KRN-0151: [351-APP-0103](../issues-done/351-APP-0103-help-reads-the-booted-slots-program.done), done; the `wifi` guide naming the station's own join, for 550-NET-0041 (its request is on the kernel session's branch): [550-APP-0104](../issues-done/550-APP-0104-the-wifi-guide-names-the-stations-own-join.done), done once 550-NET-0041 was in `main`.

Numbered on 2026-10-28: the channel the updater holds in `update status`, for 351-UPD-0025, taken into [351-APP-0029](351-APP-0029-update-in-the-shell.md) (`update.wit` 1.1 `metadata`); `run` of an installed application's path, for 750-KRN-0169: [750-APP-0109](750-APP-0109-run-takes-an-installed-applications-path.md).

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

**From:** NRL / Codex, 2026-10-26 · **For:** [900-NRL-0017](900-NRL-0017-native-runtime-and-nnd-integration.md), 900 T11; [262](262-model-runtime-on-the-machines-full-hardware.md) · **Owner to number:** APP.

Main has the KRN memory/worker APIs but no `nnd` or `idl/nn.wit`. NRL T06–T10 and their source/models/replay evidence are ready on `codex/NRL-lab`; native integration is the remaining prerequisite. Please number the APP-owned service/IDL/client part and agree its boundary with NRL. NRL supplies a checked borrowed graph/kernel/session executor under `nrl/`, with caller-owned workspace/output, deterministic work charging and cooperative owner/model-bound cancellation/deadlines. This internal Rust interface is not a stable cross-domain ABI.

The APP part needs a versioned `nn.wit`, parser-session validation and verified model identity; capability-authenticated client/session identity; caller budgets translated to native memory objects/worker/time/cancellation controls; service reserve protection and model read-only sharing; and a native smoke harness that loads a standalone checked model, runs it through nnd, cancels/exhausts a job, rejects wrong-model/other-client reuse, and runs another session after a numeric failure. Please provide actual interface names/versions and a test entry. NRL will implement its side without changing APP/KRN directories or inventing a service API. The first portable graph profile is a bounded feed-forward model; trained decoder/native cache support remains separately explicit. Neither cross-builds nor host simulation close the native criteria of T11. NRL continues host T12/T13 while this request is pending.

NRL implementation update: `nrl/crates/nrl-runtime` version 0 and `nrl/crates/nrl-lab/src/runtime_bridge.rs` are ready on `codex/NRL-lab`, with **82 host tests** in the final branch and both bare-metal target builds. [Internal contract](../nrl/specs/native-boundary-v0.md), [host evidence](../nrl/reports/native-boundary-2026-10-26.json). `Kernel`, `Program`, `Session`, `Budget` and `Control` are linked Rust types; nnd authenticates caller identity and supplies its native time/cancel/memory/worker adapter. The sample portable family is feed-forward only. T11 remains open for the APP-owned/native criteria; later host decoder/embedding work does not cover those criteria.
