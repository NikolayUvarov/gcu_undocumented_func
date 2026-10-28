# 109-KRN-0117 — A loader client that only starts helpers, for the updater's parsers

**Type:** `loader` (`loader.wit` 1.10), `init` · **Owner:** kernel session · **Priority:** P2 · **Status:** in progress (in `main` at 1b1436a; a refused `run` with the badge not yet tested) · **Blocked by:** — · **Main task:** [109](../issues-done/109-session-parsers.done) · **Constitution:** MC-3.4, MC-3.7, MC-8.5

## Problem

The updater parses each check's channel, manifest and heads in a `parse` helper of the check's own, as `download` does ([109-NET-0010](../issues-done/109-NET-0010-a-parser-per-session.done)), when it holds a loader client in `SLOT_LOADER` ([109-UPD-0017](../issues-done/109-UPD-0017-the-updaters-checks-in-their-own-parser.done)). `init` gives it none and no endpoint quota, so it uses the shared parser. A plain loader client would also let the updater start any program, which it must not hold (MC-3.7). The update track's request of 2026-10-14 is in [requests-KRN.md](requests-KRN.md).

## Plan

1. **A badge on `loader`'s clients, `BADGE_HELPERS_ONLY`** (`loader.wit` 1.10, in `mind::loader`): with it, `helper` is served as for any client ([109-KRN-0096](../issues-done/109-KRN-0096-the-loader-starts-a-helper.done)), and `program`, `launch` and every call that starts a program are refused with `denied`.
2. **`init` lends the updater that client** in `SLOT_LOADER`, with an endpoint quota of 1: the session's endpoint, which the helper serves.
3. **When the updater restarts,** `init` lends it a new one.

## Acceptance criteria

- [ ] The `update` suite: the updater's check runs its parsing in a helper that holds only its endpoint (`stat caps`), and the helper is gone when the check ends.
- [ ] A `program` call with the updater's loader client is refused with `denied` (`check`'s services group, a client with the badge).
- [x] The IDL fuzzer and the generated files follow `loader.wit` 1.10.

## Progress (2026-10-20)

Made:
- **`loader.wit` 1.10.** A client badged `BADGE_HELPERS_ONLY` (`mind::process`, 1) may list, inspect and start helpers. `run` and every session call (`begin`, the grants, the commits, `abort`) are refused with `rights`, and a lent capability is dropped. The wire is unchanged; the generated binding is regenerated.
- **`init`** lends the updater that client in `SLOT_LOADER` with an endpoint quota of 1, for the endpoint its check's parser serves. A restart keeps the client working, as for every client `init` lends.

Test. `usb_image_smoke.py`'s slots check ("x86: USB image") requires:
- the updater's slot 5 holds a client of `loader` with badge 1;
- once the updater has read the running version through it, `parse` runs alone, so the check's own parser ended.

It passes locally on x86.

Not tested yet: a refused `run` with that badge. No program can hold such a client, and `check` has no way to get one.

**The gate (2026-10-20).** Gate g0131 passed (`c23e754` on Effector's machines; the two groups with the updater suite again on `1b1436a`), and `main` is at 1b1436a. The updater suite now expects the loader's client in slot 5, and "x86: USB image" checked its badge and the check's parser ending.

## Related

[109-UPD-0017](../issues-done/109-UPD-0017-the-updaters-checks-in-their-own-parser.done), [109-KRN-0096](../issues-done/109-KRN-0096-the-loader-starts-a-helper.done), [109-NET-0010](../issues-done/109-NET-0010-a-parser-per-session.done), the stall of the updater's fetch after the cut (the kernel track's message to UPD of 2026-10-14 on `internal-dev-messages`).
