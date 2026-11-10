# Requests for the tools track (APP), not numbered yet

**Owner:** tools track · **Status:** open (2 requests waiting to be numbered: `ip` naming a card out of use, NET's of 2026-11-09; the shell asking for `keystore`'s passphrase, NET's of 2026-11-10) · **Recorded by:** the kernel track (KRN), 2026-10-07

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

Numbered on 2026-10-28: the channel the updater holds in `update status`, for 351-UPD-0025, taken into [351-APP-0029](../issues-done/351-APP-0029-update-in-the-shell.done) (`update.wit` 1.1 `metadata`); `run` of an installed application's path, for 750-KRN-0169: [750-APP-0109](../issues-done/750-APP-0109-run-takes-an-installed-applications-path.done); NRL's native `nnd` integration, for 900-NRL-0017 (900's T11): [262-APP-0112](../issues-done/262-APP-0112-nnd-and-nn-wit-stage-0.done), its design for agreement first.

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

Numbered on 2026-10-28 as [262-APP-0112](../issues-done/262-APP-0112-nnd-and-nn-wit-stage-0.done), its design ([docs/nn/runtime.md](../docs/nn/runtime.md)) for agreement first. Kept as a heading because [900-NRL-0017](900-NRL-0017-native-runtime-and-nnd-integration.md) links to it.

### NRL pinned conformance and design clarification (2026-10-29)

For existing [262-APP-0112](../issues-done/262-APP-0112-nnd-and-nn-wit-stage-0.done), NRL agrees on text object files and f64-only `nrl.execution/0`. [900-NRL-0025](../issues-done/900-NRL-0025-native-conformance-and-app-handoff.done) supplies the [pinned MIT bundle](../nrl/tests/fixtures/native-conformance-v0/bundle.json) and [handoff contract](../nrl/specs/native-conformance-v0.md). Please update your next design edition/implementation to name the canonical-writer subset, preserve exact float/typed field encoding, distinguish existing std host readers from the native parser still to implement, retain the individual 16384-value object cap, test whole-request/cumulative work preflight instead of unreachable midway work exhaustion, preserve successful output until copied, distinguish nonfinite-input refusal from internal numeric poisoning, and translate no-deadline explicitly. Also make the lifetime/reuse of a 16-bit badge explicit with KRN: retained old sessions/jobs/delegated capabilities must not become the authority of a new client after badge reuse; this needs an owner design decision and native lifecycle case. The concrete native tests and unchanged model ID/tolerance are in that contract. 262-KRN-0178 is the boot prerequisite; worker-per-job need not wait for 0179. NRL's own work remains within `nrl/`; APP owns `nnd`, `nn.wit`, native parser/glue and its design.

## The shell drops serial input past 64 events in one pass of its loop (for 000-KRN-0188)

Numbered on 2026-10-30 as [000-APP-0115](000-APP-0115-the-shell-reads-serial-input-it-can-hold.md).

## An arm for `vfs.wit` 2.6's error `conflict` in the shell (for 311-KRN-0189)

Numbered on 2026-10-30 as [311-APP-0116](../issues-done/311-APP-0116-the-shell-names-a-vfs-conflict.done).

### NRL service follow-up (2026-10-30)

APP's ALL reply of 2026-10-29 accepts the 0025 corrections in design 1.0 and owns its parser/encoders/views; there is no NRL decoder request. nnpkg/nnd/nn.wit and KRN's boot part are now in main. [900-NRL-0026](../issues-done/900-NRL-0026-portable-import-differential-check.done) checks fresh exports through the actual portable implementation without changing APP code. The remaining request stays in numbered [262-APP-0112](../issues-done/262-APP-0112-nnd-and-nn-wit-stage-0.done): nnd's two file scopes, shell client and retained/revoked/retired badges, native nncheck and named-target isolation/budget/failure/lifecycle evidence. The existing service/grant suite does not yet cover that model run. Please refresh the design page and issue progress that still say no code exists or boot grants are absent; distinguish those historical statements from the remaining native client/scopes/suite work.

## `net arp` waits a second by the clock, not by its count of tries (for the gate's `x86: network, TLS and Effector`)

Numbered on 2026-10-31 as [000-APP-0119](../issues-done/000-APP-0119-net-arp-waits-a-second-by-the-clock.done).

## The shell lends a `storefs` client to programs (for 311-KRN-0190), once `storefs` can make one that does not write

Numbered on 2026-10-31 as [311-APP-0121](../issues-done/311-APP-0121-the-shell-lends-a-storefs-client.done).

## The shell puts a running program's question about a flow to the user (for 000-NET-0049)

Numbered on 2026-10-31 as [000-APP-0120](../issues-done/000-APP-0120-the-shell-asks-the-user-about-a-programs-flow.done).

## A recovery key shown once and typed back, and a store opened with one (for 308-STO-0028)

Numbered on 2026-11-04 as [308-APP-0122](308-APP-0122-a-recovery-key-shown-once-and-typed-back.md): a program with a window of its own, `recovery`, and `--key-file` for the tests.

## The machine's name as one system setting, and `hostname` (for 750-EFF-0018 and 000-KRN-0180)

Asked on 2026-10-28 in `ALL/2026-10-28T0407Z-EFF-the-machines-name.md` (EFF with LNK) and not recorded here then. Numbered on 2026-11-05 as [000-APP-0123](../issues-done/000-APP-0123-the-machines-name-and-hostname.done), at P1 by the maintainer's word: the file is `data/machine/name`.

## The shell lends the station's reading client (for 550-NET-0058), once `KRN` gives the request bit

Asked by NET on 2026-11-05. Numbered the same day as [550-APP-0124](../issues-done/550-APP-0124-the-shell-lends-a-reading-wifi-client.done), its plan taken from this request; done on 2026-11-09.

## A script's program asked about a flow while the script waits on it (for 000-NET-0048)

Asked by NET on 2026-11-06. Numbered on 2026-11-08 as [000-APP-0125](../issues-done/000-APP-0125-a-scripts-program-asks-about-a-flow.done).

## A console program reads a secret typed at its console (for 000-NET-0048's `get -u`)

Asked by NET on 2026-11-06. Numbered on 2026-11-08 as [000-APP-0126](../issues-done/000-APP-0126-a-console-program-reads-a-secret.done): the shell reads one secret line for the program.

## A display mode chosen by hand, and two screens side by side, on the DEXP (for 177)

Asked by DRV for the maintainer on 2026-11-08, in `APP/2026-11-08T1210Z-DRV-…` on `internal-dev-messages`. The mode by hand is numbered as [177-APP-0127](177-APP-0127-a-display-mode-chosen-by-hand.md); the screens' places are [177-APP-0094](177-APP-0094-wm-over-several-monitors.md)'s. The maintainer will say when to start.

## `ip` names a card out of use (for 550-NET-0062)

**Asked by** NET (made by STO) on 2026-11-09, for [550-NET-0062](550-NET-0062-the-card-chosen-in-software.md).

- **What the stack gives** with `idl/socket.wit` 2.11:
  - `withdrawn` returns the cards that are out of use, a bit each, by the policy's line `card N off`;
  - any client may read it, the shell's too.
- **Asked of `ip`:** after a card's kind, the default and the preferred card (550-APP-0085), the word `OFF` for a card out of use, for example `CARD 0 WIRED OFF`. The card's address is gone while it is off, so its MAC line shows none.
- **Why:** the maintainer moves a machine's traffic between a wired card and the station in software (`netpolicy add card 0 off`). Today only `netcheck cards` shows which card is off.
- **Checked by** the `net` suite's two-card checks, as `ip_cards()` reads `ip` today.

## The shell asks for `keystore`'s passphrase at boot, and sets or changes it (for 308-NET-0055), once `init` lends the badge

**Asked by** NET (made by STO) on 2026-11-10, for [308-NET-0055](308-NET-0055-keystores-passphrase-without-a-tpm.md). **Waits for** [308-KRN-0205](308-KRN-0205-the-boot-waits-for-keystores-passphrase.md), which grants the shell a `keystore` client badged `BADGE_KEY_PASSPHRASE` (6, `mind::network`) and starts the shell before the services that need the device key.

- **What `keystore` gives** with `idl/keystore.wit` 1.6, in `main` since round 80:
  - `key-state`, any client's: `held`, `waiting` or `none`, answered at once;
  - `unlock(passphrase)` and `passphrase-set(old, new)`, for badge 6 alone; a passphrase is 8 to 128 bytes;
  - `denied` for a wrong one; from the third wrong one in a row, `budget` until a wait that doubles from 1 s up to 60 s has passed.
- **Asked of the shell:**
  - **at boot,** while `key-state` says `waiting`:
    - say why the boot waits, in one line;
    - read the passphrase without echo, as 000-APP-0126 reads a secret, and call `unlock`;
    - on `denied` or `budget`, say so and the wait, and ask again;
  - **a command to set or change it,** asking for the old one (none the first time) and the new one twice, each without echo, then `passphrase-set`;
  - **no passphrase on a command line, in the history or in a log,** and the shell's copies wiped once sent. The generated client wipes its call buffer (000-KRN-0226).
- **Why:** without it no task can type the passphrase, so a machine without a TPM keeps its device key unsealed on the boot disk.
- **Checked by** a suite that types it over the serial console, as it types every command. The `tls` suite's `locked_file_check` already places a sealed file from the host.
