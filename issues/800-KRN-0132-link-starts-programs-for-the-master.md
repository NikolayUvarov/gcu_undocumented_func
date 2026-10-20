# 800-KRN-0132 — `link` holds an application's loader client, to start programs for the master

**Type:** `init` · **Owner:** kernel session · **Priority:** P1 · **Status:** made, in `fast-test` (2026-10-20); the gate is left · **Blocked by:** — · **Main task:** [800](800-mind-link.md) (the link track's) · **Constitution:** MC-2.3, MC-3.4, MC-3.11

## Problem

The maintainer asked on 2026-10-20 that the agent on the other end of the RS-485 link can test a program on MIND Core without a reboot.
- What works already: the agent puts the program in `data/link/` (`mindlink put`). `loader` runs a path with a `/` (`path_for`), so `run data/link/<name>.elf` starts it.
- What is missing: only someone at the machine's keyboard can type that. `link` cannot start a program.
- Why not the shell's interface: `idl/shell.wit`'s `run` refuses programs and scripts, and a command that changes the machine waits for the user's answer in the shell's window.

## Plan

1. **`init` lends `link` a loader client in `SLOT_LOADER` (5).** It is the client an application holds: no badge, so none of the loader's privileged requests.
   - A program `link` starts gets an application's standard grants and what `link` itself grants it in a launch session, which can only be what `link` holds.
   - So the cable gains no authority beyond what it has with `data/link/`: it may start a program, as any application may.
2. **The link track's part** ([requests-LNK.md](requests-LNK.md), "Starting a program for the master"): a request in `link` and `mindlink` that starts a program from `data/link/` through a launch session, and returns its output over the console channel.
3. **Stopping a program** that does not end is not part of this task. It needs a grant to end the tasks `link` started, which is still to be decided (with [750-KRN-0121](750-KRN-0121-a-lifecycle-client-for-named-services.md)).

## Acceptance criteria

- [ ] `stat caps` of `link` shows exactly four grants: `usb_serial`'s client (2), the scope of `data/link/` (3), `loader`'s client (5), the log reader (12). This is checked in the `link` suite.
- [ ] With the link track's request: the `link` suite puts a program into `data/link/`, starts it through `mindlink`, and reads its output.

## Progress (2026-10-20)

`init`'s `link` arm lends `loader`'s keeper client in `SLOT_LOADER`. `loader` starts after `link`, so calls fail with `ERR_PEER` until it runs. The `link` suite checks the four grants. It is in `fast-test`; the suites run after the merge.

## Related

[800-KRN-0128](800-KRN-0128-init-starts-usb-serial-and-link.md) (`link`'s other grants), [800-LNK-0003](800-LNK-0003-the-link-service-on-mind-core.md), [requests-LNK.md](requests-LNK.md), [750-KRN-0121](750-KRN-0121-a-lifecycle-client-for-named-services.md).
