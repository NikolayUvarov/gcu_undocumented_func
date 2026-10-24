# 750-KRN-0169 — The loader checks an installed application before it runs it; the gateway's `data/apps/` scope

**Type:** `loader`, `init` · **Owner:** `KRN` · **Priority:** P2 · **Status:** open · **Blocked by:** — (the install itself is [750-EFF-0008](750-EFF-0008-application-packages.md)) · **Main task:** [750](750-effector-agent.md) · **Constitution:** MC-9.1, MC-9.2, MC-3.11

Recorded by the kernel track on 2026-10-24, for the loader's part of [750-UPD-0019](750-UPD-0019-signed-application-packages.md) (draft 0.2, "The loader's check"). The Effector server project asked for it (`ALL/2026-10-24T0904Z-EFF-server-accepts-package-draft-0-2-point-3.md`, point 3).

## Problem

- **What the loader does now.** It runs any `*.elf` it is asked to, from the booted slot or the volume's root. Nothing checks an application: the profile says "not met" for that (Article 9).
- **What is coming.** The Effector gateway is to install signed packages under `data/apps/<name>/<version>/` (750-EFF-0008, 750-UPD-0019). A package checked once at install time can be changed afterwards by anyone who can write the disk.
- **What the gateway lacks.** 750-KRN-0120 gives `effector_gw` one writable scope, the policy's `deliver` root. It has none for `data/apps/`, so it cannot install there.

## Plan

1. **The check before a run.** Before the loader runs a program from `data/apps/`, it checks it with `apppkg` (750-UPD-0019's shared routine):
   - the stored `MANIFEST.SIG` against the keys the owner trusts for that name: the `package-key` lines of `data/effector/policy`, read whole or not at all;
   - the program's size and SHA-256 against its `file` line;
   - that the requests in its `.mind_request` are no more than the line says.

   Any mismatch, a key no longer trusted or a policy that cannot be read: the program does not run, and the loader's log says why.
2. **`data/apps/<name>/current`.** The loader runs the version `current` names, as 750-UPD-0019 lays it out; nothing else under `data/apps/` runs.
3. **The gateway's scope.** At each start `init` gives `effector_gw` a writable scope of `data/apps/`, beside its `deliver` scope, in a slot of its own. A new slot constant is an ABI change: it waits for the next ABI version and says so there.
4. **Applications elsewhere are unchanged.** Those of the booted slot and the volume's root are checked as today: by the boot manifest at boot for the services, not at all for applications (that is a separate step).

## Acceptance criteria

In QEMU, x86 and aarch64, with a package of the public test key that the host lays out under `data/apps/` as the gateway will:
- it runs while its key is trusted for its name;
- it is refused, with the reason logged, when one byte of the program changes, when its `MANIFEST` changes, when the key line is removed or names another application, and when it asks for more than its line says;
- the gateway's stand-in writes under `data/apps/` and is refused outside it.

What this does not show: an install by the gateway (750-EFF-0008), or a check of applications outside `data/apps/`.

## Related

[750-UPD-0019](750-UPD-0019-signed-application-packages.md), [750-EFF-0008](750-EFF-0008-application-packages.md), [750-KRN-0120](../issues-done/750-KRN-0120-writable-scopes-for-a-service.done), [351-UPD-0009](351-UPD-0009-rollback-policy-and-key-roles.md).
