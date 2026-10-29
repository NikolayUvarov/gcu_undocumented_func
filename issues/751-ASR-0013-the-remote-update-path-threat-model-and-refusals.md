# 751-ASR-0013 — The remote update path: its threat model, and tests that it is refused whenever the permission is off

**Type:** assurance · **Owner:** `ASR` · **Priority:** P1 · **Status:** in progress (2026-10-29) · **Blocked by:** the parts of [751](751-system-updates-at-a-developers-request.md) (`EFF`, `UPD`, `KRN`, `APP`) for the tests; nothing for the model · **Main task:** [751](751-system-updates-at-a-developers-request.md) · **Constitution:** MC-12.6, MC-7.5, MC-9.3, MC-9.4, MC-3.11, MC-10.3

## Problem

[751](751-system-updates-at-a-developers-request.md) lets a developer agent update a machine's system through Effector while the owner's `remote-update` is on. This is the most powerful remote path MIND Core will have, so its limits need writing down and testing.

## Plan

1. **The threat model,** [docs/assurance/remote-update-threats.md](../docs/assurance/remote-update-threats.md), apart from [351-ASR-0006](351-ASR-0006-update-threat-model.md)'s document, which is not in `main` yet. It covers:
   - **Who can turn the permission on:** only the console, the disk, and the cable with `link-remote-update`. Never the server, a policy line, the Mind or a program.
   - **What a server holding the agent's token can do while it is on:** install any release signed with keys the machine trusts.
   - **The test keys' risk.** With the public test release and boot keys trusted, as the maintainer's development disks do today, anyone who reaches the server installs any code.
     - This is acceptable only on the maintainer's development machines.
     - A disk that trusts only the maintainer's own release key is required before the permission is used anywhere else.
     - The model says so, and `docs/profile` must not claim more.
   - **Replay and expiry** (MC-7.5): the server's command and its term; a bundle sent twice; a release older than the floor (MC-9.4).
   - **What stays the same:** the trial boot and the fall-back (MC-9.3).
   - **The audit** (MC-10.3).
2. **The refusal tests,** QEMU, x86 and aarch64, once the parts exist:
   - with the word off, the gateway's update is refused before a byte reaches `updater`;
   - `set-mode` of `remote-update` on, from the Mind's badge, is refused, and off is taken;
   - with `link-remote-update` off, the link's `mode remote-update on` is refused;
   - a bundle signed with a key the machine does not trust is refused by `updater` with the word on;
   - a release below the floor is refused with the word on.

## Progress

- **2026-10-29, the model's first version** (0.1): [docs/assurance/remote-update-threats.md](../docs/assurance/remote-update-threats.md).
  - Every attack is listed with the check asked of its track and the test asked here.
  - The test keys' risk is stated: on the development disks the path installs anyone's code.
  - Open: refusing the word on a disk that trusts the test keys.
  - It is a design until the parts exist (MC-12.3). Review by `EFF`, `UPD` and `KRN` is asked on `internal-dev-messages`.

## Acceptance criteria

- [ ] The threat model's section, reviewed by `EFF`, `UPD` and `KRN` on `internal-dev-messages`.
- [ ] The refusal tests above in the `effector`, `update` or `link` suites, passing on x86 and aarch64.

## Related

[751](751-system-updates-at-a-developers-request.md), [351-ASR-0006](351-ASR-0006-update-threat-model.md), [351-ASR-0005](351-ASR-0005-power-loss-during-update.md).
