# 000-KRN-0075 — init waits for the policy broker with a limit

**Type:** kernel (`init`) · **Owner:** kernel session · **Priority:** P3 · **Status:** open · **Blocked by:** — · **Roadmap:** track A · **Constitution:** MC-6.3 (a failed service is contained), MC-2.13 (bounded waits)

## Problem

Before it starts `updater`, `init` asks `netpolicy` for the updater's network grant (`flow` in `init/src/main.rs`: `netpolicy::prepare`, then `take`). The calls have no time limit.

`netpolicy` in turn calls `netstack` and reads its policy through `vfs_server`. If any of them stops answering, `init` waits for ever, and neither `updater` nor the shell starts.

The MacBook Pro's boot of 2026-10-11 stopped at the same point, though for another reason. `bcm_wifi`'s stage 2 froze the machine (550-DRV-0023; fixed there). That boot was the first with an `updater` image on a machine without a network card. Before it, `init` logged "updater NOT STARTED: NO IMAGE" and went on to the shell.

## Plan

- **The calls.** `init` makes them within a limit, for example 5 s. `libmind`'s IDL runtime gets `wire::with_timeout`, which the tools track added for word calls in 211-APP-0044, for buffer calls and calls that receive a capability as well.
- **The log.** Before asking, `init` logs that it asks the broker. It logs when the broker does not answer, and then starts `updater` without a network grant.

## Acceptance criteria

- **A broker that never answers.** A QEMU check with a stand-in for it: `init` logs the limit and the shell starts.
- **The updater suite** passes as before.

## Related

[351-KRN-0022](../issues-done/351-KRN-0022-updater-grants.done) (the updater's grants), [550-DRV-0023](550-DRV-0023-bcm4331-microcode-runs.md) (the freeze).
