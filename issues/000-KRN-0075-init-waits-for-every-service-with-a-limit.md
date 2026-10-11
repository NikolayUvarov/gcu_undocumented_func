# 000-KRN-0075 — init waits for every service with a limit

**Type:** kernel (`init`, the IDL runtime) · **Owner:** kernel session · **Priority:** P1 · **Status:** in progress (made and tested in QEMU x86; the gate left) · **Blocked by:** — · **Roadmap:** track A · **Constitution:** MC-6.3 (a failed service is contained), MC-2.13 (bounded waits)

## Problem

`init` called other services with no time limit:

- the policy broker, for the updater's network grant (`flow`: `netpolicy::prepare`, then `take`);
- the block drivers, to find the block store's disk (`store_disk`);
- `vfs_server`, for the hardware report, the boot volume's mount check, and the flush before a restart.

If any of them stopped answering, `init` waited for ever. Then neither the services after it nor the shell started, and `init` no longer supervised the others.

`netpolicy` in turn calls `netstack` and reads its policy through `vfs_server`. So one stuck service anywhere on that path stopped the boot.

The MacBook Pro's boot of 2026-10-11 stopped at that point, though for another reason: `bcm_wifi`'s stage 2 froze the machine (550-DRV-0023; fixed there, the recovery is 000-KRN-0076). That boot was the first with an `updater` image on a machine without a network card. Before it, `init` logged "updater NOT STARTED: NO IMAGE" and went on to the shell. The maintainer's word: the kernel must not wait, and must not hang because of such things.

## Plan

- **The IDL runtime.**
  - `libmind`'s `wire::with_timeout` (the tools track's, 211-APP-0044, taken word for word) bounded word calls.
  - It now bounds calls with a buffer (`call_buffer`) and calls that receive a capability (`call_receiving`) as well.
- **`init`.**
  - Runs its boot and its service loop under a limit of 10 s a call (`CALL_LIMIT_MS`).
  - The boot volume's mount check and the flush before a restart have 60 s (`MOUNT_LIMIT_MS`): a slow USB stick takes seconds.
  - A service that does not answer is logged and passed over: `[INIT] updater: THE POLICY BROKER DID NOT ANSWER IN 10 S`, or `<driver> DID NOT ANSWER IN 10 S: NOT THE BLOCK STORE'S DISK`. The updater then starts without a network grant.
- **Not changed.** `init`'s own log lines still go to `logd` under the same limit, so a stuck `logd` would slow the boot by 10 s a line.

## Acceptance criteria

- **A broker that never answers, on a machine without a network card.** In the `updater` suite, `init` logs the limit, starts the updater without a network grant, and starts the shell. The broker is the updater's stand-in in `netpolicy`'s place, which never receives.
- **The suites** pass as before, on x86 and aarch64.
- **The host tests** (`idl_host`) show the limit on word calls. Buffer calls are covered by the `updater` suite: `netpolicy::prepare` is one.

## Progress

**2026-10-11: made.**

- `libmind/src/idl/wire.rs` and `init/src/main.rs` as above. The `updater` suite boots a second image without a network card, with the silent broker. The host tests check it on word calls. A host test of buffer calls was dropped: the limit is one global, and it raced the word-call test running beside it.
- Run locally on x86:
  - the `updater` suite passes both boots;
  - the `idl` and `idl_fuzz` host tests pass, serially;
  - the group "x86: boot, display, network, TLS, shell, memory, clock" passes.

## Related

- [351-KRN-0022](../issues-done/351-KRN-0022-updater-grants.done) (the updater's grants)
- [550-DRV-0023](550-DRV-0023-bcm4331-microcode-runs.md) (the freeze)
- [000-KRN-0076](000-KRN-0076-a-frozen-machine-restarts-and-starts-safely.md) (recovering from a freeze)
