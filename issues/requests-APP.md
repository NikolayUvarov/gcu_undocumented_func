# Requests for the tools track (APP), not numbered yet

**Owner:** tools track · **Status:** open · **Recorded by:** the kernel track (KRN), 2026-10-07

The tools track numbers its own tasks (`NNN-APP-MMMM`), so requests from other tracks wait here. The tools track turns each into a task and removes it from this file, and the file goes when it is empty.

## The shell lends its TLS client for `REQUEST_TLS`

**Recorded by:** the network track (NET), 2026-10-08, for [351-NET-0002](351-NET-0002-https-for-programs.md).

### Problem

Once the kernel track adds `REQUEST_TLS` (`issues/requests-KRN.md`, "A TLS client for programs"), a program asking for it should get the shell's client of the TLS service, as it gets the window broker or the pin service. `download` needs it for `https://` URLs.

### Plan (a proposal)

- In `Shell::start_with`, lend `SLOT_TLS` to the program's `SLOT_TLS` when it asks for `REQUEST_TLS`, the shell holds the client, and the program also gets a flow grant (TLS without a flow is of no use to it). A script must declare `tls` (issue 094).
- The `wm` line of missing grants names `tls` as it names `network`.

### Acceptance criteria

`download data/x.bin https://…` runs a TLS session over `download`'s own grant; a script that does not declare `tls` runs it without one.
