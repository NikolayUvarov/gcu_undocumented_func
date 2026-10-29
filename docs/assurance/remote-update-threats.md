# The remote update's threats

**Version:** 0.1 (2026-10-29, [751-ASR-0013](../../issues/751-ASR-0013-the-remote-update-path-threat-model-and-refusals.md)) · **Constitution:** [v1.6](../../constitution/EN/MIND_CORE_Constitution_v1.6.md) MC-12.6, MC-7.5, MC-9.3, MC-9.4, MC-3.8, MC-3.11, MC-10.3, MC-12.3

This document covers a developer agent installing a release on a machine through Effector, under the owner's permission `remote-update` ([751](../../issues/751-system-updates-at-a-developers-request.md)). It lists what an attacker could do with that path, what is meant to stop each attack, and what nothing stops.

**This is a design, not a guarantee (MC-12.3).** On 2026-10-29 none of 751's parts exists:
- `init`'s words (KRN);
- `updater`'s client for the gateway (UPD, 750-UPD-0020);
- the shell's switches (APP);
- the agent's and the gateway's side (751-EFF-0023).

The "Stopped by" column names the check each part is asked for. The "Test" column names the test asked for in 751-ASR-0013. Each row becomes a claim only when its test passes on the configuration it names.

The update path as it stands (`updater`, the slots, the signed manifest and channel) is [351-ASR-0006](../../issues/351-ASR-0006-update-threat-model.md)'s. Its document is on `claude/351-ASR-0006-update-threats`, not yet in `main`. This one adds only what the remote path changes.

## What the path is

1. **The owner turns `remote-update` on,** a word of `data/mode.conf`. It counts only in the development or maintenance mode.
2. **The Effector server sends `update_agent`** with a bundle's size and SHA-256.
3. **The agent downloads the bundle,** and the gateway streams it to `updater` through its badge.
4. **`updater` installs it.**
   - It asks `init` whether the word is on.
   - It checks the bundle as any release: the manifest's signature with a boot key, the channel's with a release key, and the version floor.
   - It writes the other slot and restarts into it on trial.

## The keys decide everything

**While `remote-update` is on, whoever can make the server send a command can install any release the machine's keys accept.**
- **The development disks** (the DEXP and the MacBook Pro on 2026-10-29) trust the **test** boot and release keys.
- **The test keys' seeds are public text** ([publishing.md](../update/publishing.md), "Keys").
- **So on those disks the path installs anyone's code.** That includes a person who learns the agent's server account, the server's operator, or whoever controls the server.

Hence:
- **The test keys are for the maintainer's development machines only,** and only while 751's phase of active development lasts.
- **Before `remote-update` is turned on anywhere else,** the disk must trust only the maintainer's own release key and boot key, held outside the repository. That disk is written with `$MIND_RELEASE_KEY`, `$MIND_BOOT_SIGNING_KEY` and their public halves (publishing.md).
- **Nothing in 751 enforces this.** `init` does not refuse the word on a disk that trusts the test keys. Making it refuse is open, below. Until then, `docs/profile` must not describe the path as protected.

## Attacks and what is meant to stop them

| Attack | Stopped by (asked) | Test (asked, 751-ASR-0013) | Not covered |
|---|---|---|---|
| **The server turns the permission on** | Only `BADGE_MODE` (the shell, at the console) and the disk set the word; no Effector badge, policy line or command can (KRN, MC-12.6) | `set-mode` of `remote-update` from the gateway's and the agent's badges refused | — |
| **The Mind turns it on** | `BADGE_MIND` may turn it off only (KRN) | `set-mode` on from `BADGE_MIND` refused, off taken | — |
| **The cable turns it on without the owner's leave** | The link's console takes `mode remote-update` only while `link-remote-update` is on, which only the console or the disk sets (APP, KRN) | `link` suite: refused without the word; `mode link-remote-update` from the link always refused | Whoever holds the cable while the owner's word is on can switch it: that is the owner's choice |
| **An install with the permission off** | `updater` asks `init` at `install-begin` and again at `install-commit` and refuses (UPD) | `update` and `effector` suites: refused, nothing written to the other slot | A switch-off during a running install: the commit check refuses it, but the bytes already streamed are discarded only by the next install |
| **The gateway installs on its own** | The gateway holds no file scope `updater` reads; it can only stream to `updater`, which checks the word itself (UPD, KRN) | `effector` suite with the word off | The gateway's own code is trusted to stream what the agent downloaded, not to inspect it |
| **A release signed with an unknown key** | `updater`'s checks, unchanged: the manifest with a trusted boot key, the channel with a trusted release key (351) | `update` suite: a bundle signed with another key refused with the word on | **The test keys** (above): with them trusted, every release is "known" |
| **Rollback to an older, vulnerable release** | The version floor in the boot records, raised by `updater` (351-UPD-0024, MC-9.4) | `update` suite: a bundle below the floor refused with the word on | The floor is on the disk, not in hardware: someone with the disk can lower it (351-ASR-0006's "boot-record tampering") |
| **A bundle that breaks the machine** | The other slot, the trial boot, `init`'s confirmation and the fall-back (351-UPD-0006, 351-KRN-0014, MC-9.3) | `update` suite: an unconfirmed trial falls back | A release that boots and confirms but misbehaves later stays until the next update |
| **A replayed command, or one long after it was sent** (MC-7.5) | The server's command has an expiry (`expires_at`); the agent journals a command before it runs it and answers a redelivery with the saved answer (750-EFF-0006) | `effector` suite: a redelivered command not run again (exists for other actions) | A bundle replayed under a new command installs again if it is above the floor; the floor stops only older ones |
| **A huge or endless bundle** | The size announced in the command, checked as it streams; `updater`'s limit on a slot | `effector` suite: a size that differs refused | The download's overall time is bounded by the command's term only |
| **A bundle changed in transit** | TLS to the server with a pinned key (750-NET-0020), then the announced SHA-256, then the manifest's signature | `effector` suite: a SHA-256 that differs refused | — |
| **No trace of who installed what** (MC-10.3) | The gateway's audit line (command, initiator, reason, result) and `updater`'s log of the version and the badge | `effector` suite reads both | The audit lives on the machine; a release that wipes `data/` takes it along |

## Open

1. **Refusing the word on a disk that trusts the test keys,** or showing that loudly. It needs a decision by KRN and the maintainer: the development disks need the word, and the test keys, today.
2. **What the owner sees while the word is on:**
   - the shell's notice (APP);
   - the agent's mark, red (751-EFF-0023);
   - nothing outside the console and the screen.
3. **The server's own security** (accounts, keys, its operators) is outside MIND Core. The table assumes only that the server can be wrong or hostile.
