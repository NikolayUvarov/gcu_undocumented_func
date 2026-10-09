# Requests for the tools track (APP), not numbered yet

**Owner:** tools track · **Status:** open · **Recorded by:** the kernel track (KRN), 2026-10-07

The tools track numbers its own tasks (`NNN-APP-MMMM`), so requests from other tracks wait here. The tools track turns each into a task and removes it from this file, and the file goes when it is empty.

## `update` in the shell and `msh` (351, phase 2)

**Recorded by:** the kernel track (KRN), 2026-10-08, for main task [351](351-self-update.md) at the maintainer's request.

### Problem

The `updater` service ([351-UPD-0007](351-UPD-0007-updater-service.md)) will have an interface (`idl/update.wit`) but no command.

### Plan (a proposal; the tools track decides)

- `update check | fetch | apply | status | rollback` in the shell, and the same in `msh` under `requires: lifecycle`.
- `apply` asks for confirmation and names the versions.
- `status` shows the running, staged, trial and last-known-good versions and the last error.
- `sysmon` or `top` may show a staged update.

### Acceptance criteria

The commands drive the updater against the test server in QEMU, and the tools suite checks them.

## A Wi-Fi setup program (550)

**Recorded by:** the kernel track (KRN), 2026-10-08, for main task [550](550-network-on-real-hardware.md) at the maintainer's request.

### Problem

With the 802.11 station that the network track is asked for ([requests-NET.md](requests-NET.md)), the system could join a Wi-Fi network, but nothing lets a person choose one and type its passphrase.

### Plan (a proposal; the tools track decides)

- `wifi` (proposed name): a program in a `wm` window or on a full screen, and the same as shell commands. It:
  - lists the networks found with their name (SSID), signal, security (open, WPA2, WPA3, enterprise) and channel, and refreshes the list;
  - connects with a passphrase typed without echo;
  - offers to remember the passphrase, and stores it through `keystore`, never in a file;
  - shows the state (connecting; connected, with the address; failed, with the reason) and forgets a remembered network.
- It talks only to the station's Wi-Fi configuration interface. It never sees frames, or keys once the passphrase is handed over.

### Acceptance criteria

The tools suite checks the program against a station stand-in with fixed scan results (QEMU has no Wi-Fi). On the MacBook Pro, it lists the networks around and joins a WPA2-PSK one.

## The shell lends its TLS client for `REQUEST_TLS`

**Recorded by:** the network track (NET), 2026-10-08, for [351-NET-0002](351-NET-0002-https-for-programs.md).

### Problem

Once the kernel track adds `REQUEST_TLS` (`issues/requests-KRN.md`, "A TLS client for programs"), a program asking for it should get the shell's client of the TLS service, as it gets the window broker or the pin service. `download` needs it for `https://` URLs.

### Plan (a proposal)

- In `Shell::start_with`, lend `SLOT_TLS` to the program's `SLOT_TLS` when it asks for `REQUEST_TLS`, the shell holds the client, and the program also gets a flow grant (TLS without a flow is of no use to it). A script must declare `tls` (issue 094).
- The `wm` line of missing grants names `tls` as it names `network`.

### Acceptance criteria

`download data/x.bin https://…` runs a TLS session over `download`'s own grant; a script that does not declare `tls` runs it without one.
