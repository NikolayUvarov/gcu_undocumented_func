# 800-KRN-0137 — `link` may make one endpoint, for its programs' output

**Type:** `init` · **Owner:** kernel session · **Priority:** P1 · **Status:** made, in `fast-test` (2026-10-21); the gate is left · **Blocked by:** — · **Main task:** [800](800-mind-link.md) (the link track's) · **Constitution:** MC-3.13

## Problem

The link track's request of 2026-10-20 (`internal-dev-messages`, `KRN/2026-10-20T2029Z-LNK-link-needs-an-endpoint-quota-of-one.md`), for 800-LNK-0009:
- **How output reaches `link`.** A program prints into `SLOT_CONSOLE` (`mind::output`), and `link` grants each program it starts a send copy of an endpoint of its own there, as `console` does.
- **Why it fails.** `init` gives `link` no endpoint quota, so `Endpoint::create()` fails with `ERR_LIMIT` and RUN is refused.

## Plan

`init` gives `link` a quota of one endpoint, as the updater has (`"link" => Quota { endpoints: 1, ..Quota::default() }`).
- No grant, no capability, no ABI change.
- `link` makes the endpoint only when `link.conf` holds `run`.

## Acceptance criteria

- [ ] The link track's `link` suite (on its branch, 800-LNK-0009) starts a program from `data/link/` through `mindlink run` and reads its output to its end, on an image with this change.
- [ ] The gate passes.

## Related

[800-KRN-0132](800-KRN-0132-link-starts-programs-for-the-master.md) (`link`'s loader client), 800-LNK-0009 (RUN), [docs/profile/bootstrap.md](../docs/profile/bootstrap.md).
