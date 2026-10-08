# Requests for the tools track (APP), not numbered yet

**Owner:** tools track · **Status:** open · **Recorded by:** the kernel track (KRN), 2026-10-07

The tools track numbers its own tasks (`NNN-APP-MMMM`), so requests from other tracks wait here. The tools track turns each into a task and removes it from this file, and the file goes when it is empty.

## Clocks ask the RTC service 10 times a second

### Problem

`clock` calls `mind::rtc::seconds_since_midnight()` after every `wait_or_exit(100)`: 10 IPC calls a second per clock.

On x86 each answer is about 14 CMOS port accesses, each a system call. With 60 clocks that came to 8 700 port system calls a second. On aarch64 with 120 clocks, the RTC service's queue was full all the time. A `date` from the shell waited 8.5 s for a place (`000-KRN-0010` covers the kernel's side).

`sysmon` reads `STAT_TASKS` and `STAT_ENDPOINTS` every 100 ms. 171-KRN-0009 made both one pass, but they still cost about a millisecond each under the lock with 170 tasks.

### Plan (a proposal; the tools track decides)

- A clock could read the RTC once and count seconds from `CLOCK` (monotonic nanoseconds), reading the RTC again every minute or so.
- `sysmon` could sample endpoints less often than tasks.

### Acceptance criteria

The RTC service's load does not grow by 10 calls a second with each clock.

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


## The shell lends its TLS client for `REQUEST_TLS`

**Recorded by:** the network track (NET), 2026-10-08, for [351-NET-0002](351-NET-0002-https-for-programs.md).

### Problem

Once the kernel track adds `REQUEST_TLS` (`issues/requests-KRN.md`, "A TLS client for programs"), a program asking for it should get the shell's client of the TLS service, as it gets the window broker or the pin service. `download` needs it for `https://` URLs.

### Plan (a proposal)

- In `Shell::start_with`, lend `SLOT_TLS` to the program's `SLOT_TLS` when it asks for `REQUEST_TLS`, the shell holds the client, and the program also gets a flow grant (TLS without a flow is of no use to it). A script must declare `tls` (issue 094).
- The `wm` line of missing grants names `tls` as it names `network`.

### Acceptance criteria

`download data/x.bin https://…` runs a TLS session over `download`'s own grant; a script that does not declare `tls` runs it without one.
