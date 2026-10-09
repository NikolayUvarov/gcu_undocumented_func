# Requests for the kernel track (KRN), not numbered yet

**Owner:** kernel track · **Status:** open · **Recorded by:** the tools track (APP), 2026-10-06

The kernel track numbers its own tasks (`NNN-KRN-MMMM`), so requests from other tracks wait here. The kernel track turns each into a task and removes it from this file. The file is kept while empty because other issues link to it; a new request goes below this line.

## A TLS client for programs (`REQUEST_TLS`)

**Recorded by:** the network track (NET), 2026-10-08, for [351-NET-0002](351-NET-0002-https-for-programs.md) (HTTPS for `download` and the updater).

### Problem

Only the shell holds a client of the TLS service (`SLOT_TLS`). A program cannot ask for one: there is no request flag. So `download` (351-NET-0001) refuses `https://`, and the updater could not fetch a release over HTTPS either. Lending the client is safe as far as the network goes: `tls` runs a session over the flow the client lends it (`attach`), so the program reaches only what its own flow grant allows.

### Plan (a proposal; the kernel track decides)

- `REQUEST_TLS` in `libmind::process`: the shell's TLS client in the program's `SLOT_TLS` (slot 20, the slot the shell keeps it in, as `REQUEST_GPIO` does with `SLOT_GPIO`).
- The shell lends it only to a program that also gets a flow grant (`issues/requests-APP.md`); init's grant to `updater` stays with [351-KRN-0022](351-KRN-0022-updater-grants.md) (split from [351-KRN-0014](../issues-done/351-KRN-0014-trial-boot-and-confirmation.done)).

### Acceptance criteria

A program built with `REQUEST_TLS | REQUEST_NETWORK` finds an endpoint in `SLOT_TLS` and completes a TLS session over its own grant; one without the request finds none.
