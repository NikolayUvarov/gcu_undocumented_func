# 751 — System updates at a developer's request through Effector, under a permission only the machine's owner turns on

**Type:** main task · **Owner:** `EFF` track · **Priority:** P1 · **Status:** open (2026-10-29) · **Blocked by:** the parts below: `KRN`, `UPD`, `APP`, and the Effector server · **Roadmap:** stage V (remote capabilities through a gateway) · **Constitution:** MC-12.6, MC-7.5, MC-9.3, MC-9.4, MC-8.5, MC-3.8, MC-3.11, MC-10.3

## Problem

The maintainer (2026-10-29): a developer agent must be able to update a machine's system through Effector, for tests.
- **When it is on.** At least during active development.
- **After that it is blocked.** The permission is then turned on only from inside the machine, at the user's request.
- **By the cable too,** if that action is allowed.

Today a release reaches a machine only over the cable: `scripts/link_update.py` puts it into `data/link/rel/`, the source in `update.txt`. The cable reaches one machine at a time, at about 46 KB/s, and a slot update takes 10 minutes or more.

Effector cannot do it, and its parts are not built:
- **The agent may not replace the system, rightly.** It is part of the signed image. Only `updater` installs a release, with its checks, the other slot, a trial boot and a fall-back (351).
- **The planned link from Effector to `updater` waits:**
  - 750-EFF-0009 has the server's request check the owner's channel;
  - it waits for `UPD`'s 750-UPD-0020, a client of `updater` for the gateway.
- **`deliver_file` cannot carry a release:**
  - it writes under one delivery root, `data/inbox/` on the MacBook, which `updater` does not read;
  - a release is about a hundred files in directories, and the gateway makes none.

## The permission

1. **Its name:** a new word of `data/mode.conf`, `remote-update`, kept across restarts like the modes.
   - **What it allows:** the Effector gateway may hand `updater` a release the server sent, and `updater` may install and apply it.
   - **What it does not allow:** anything else. The release is still checked as every other is:
     - the manifest's signature with a boot key, and the channel's with a release key, both keys the machine already trusts;
     - the version floor (MC-9.4);
     - the other slot, the trial boot and the fall-back (MC-9.3).
   - **When it counts:** only with `development` or `maintenance` on, the modes the agent runs in (750-EFF-0013).
2. **Who turns it on:**
   - the owner at the console, with `mode remote-update on` and a confirmation;
   - the disk's file as written (`build_and_write.sh --mode=…,remote-update`).
3. **Who turns it off:**
   - the same;
   - the Mind (`BADGE_MIND`), which may only turn it off (MC-12.6).
4. **Never the server.**
   - No Effector command and no policy line can turn it on.
   - The owner's Effector policy may still refuse an update with it on: the policy's `update` line.
5. **The cable, only if the owner allowed it.** A second word, `link-remote-update`:
   - **who sets it:** the console or the disk, never the cable;
   - **what it allows:** with it, and with `dev` and `link-console`, a line of the link's console may turn `remote-update` on or off;
   - **without it:** the link's lines are refused `mode`, as now (800-APP-0108).
6. **The phases:**
   - **During active development:** the maintainer's disks are written with `remote-update`. A developer agent then updates them without asking each time.
   - **Afterwards:** the default disk is written without it, and only the owner turns it on, as above. When that switch happens is the maintainer's decision, recorded here when it is made.
7. **What shows it:**
   - the `mode` command and the notice above the first prompt;
   - the Effector agent's mark on the screen, red while it is on (750-EFF-0015);
   - every installed release in the system log and the gateway's audit, with who asked and why (MC-10.3).

## The way a release goes

1. **The developer builds and uploads it.** The release is built and signed as now (`scripts/release.py`), as one file (a bundle). It is uploaded to the Effector server.
2. **The server sends a command** to the agent with the bundle's size and SHA-256.
3. **The agent downloads it** in pieces, as `deliver_file` does. It writes nothing of it itself.
4. **The gateway checks and hands it over.**
   - It checks the policy.
   - It streams the bundle to `updater` through the gateway's client of `updater` (750-UPD-0020).
5. **`updater` installs it.**
   - It refuses unless `remote-update` is on: it asks `init` at that moment.
   - It checks the bundle as it checks any source and writes the other slot.
   - It applies with a trial boot.
6. **After the restart, the agent answers the command:**
   - the new version, confirmed;
   - or the fall-back and its reason.

   The answer is kept in its journal across the restart.

## The parts

| Track | Part | Record |
|---|---|---|
| `EFF` | The agent and the gateway: the command, the download, the stream to `updater`, the answer across the restart, the heartbeat's update state, the mark; contract 1.4 | [751-EFF-0023](751-EFF-0023-the-agent-installs-a-developers-release.md); [750-EFF-0009](750-EFF-0009-system-update.md) stays for the owner's channel |
| `UPD` | `updater`: the gateway's badge with `install` (a streamed bundle) and `status`, refused unless `remote-update`; the bundle's format in `scripts/release.py` | 750-UPD-0020 extended, asked on `internal-dev-messages` (2026-10-29) |
| `KRN` | `init`: the words `remote-update` and `link-remote-update`, `init.wit` 1.11, the Mind's right only to turn it off; the gateway's client of `updater` (a badge, an ABI step) | asked on `internal-dev-messages` (2026-10-29) |
| `APP` | The shell: `mode remote-update on` and `off`, and the same for `link-remote-update`, at the console with a confirmation; the link's console takes `mode remote-update` only with `link-remote-update`; the notice; `wm`'s Settings | asked on `internal-dev-messages` (2026-10-29) |
| `LNK` | `build_and_write.sh --mode=` takes both words; the `link` suite checks the cable's switch both ways | [751-LNK-0030](751-LNK-0030-the-remote-update-words-on-the-disk-and-over-the-cable.md) |
| `ASR` | The threat model of this path, and the tests that it is refused when the permission is off | [751-ASR-0013](751-ASR-0013-the-remote-update-path-threat-model-and-refusals.md) |
| The Effector server | `update_agent` for MIND endpoints with an uploaded bundle; its result after the restart | its inbox (2026-10-29) |

## Acceptance criteria

- [ ] **QEMU, x86 and aarch64:**
  - with `remote-update` on, a release sent through the Effector fixture is installed, confirmed in the other slot, and the command answered with its version;
  - with it off, the same command is refused, and nothing is written to the other slot;
  - the server cannot turn the permission on, and the link can only with `link-remote-update`.
- [ ] **On the DEXP and the MacBook Pro, through hesere:** a developer's release installed and confirmed. Evidence for each machine only (MC-12.1).
- [ ] **The docs:**
  - `docs/effector/README.md` and `protocol.md`;
  - `docs/update/updater.md`;
  - `docs/tools/README.md` (`mode`);
  - `docs/write-disk.md` (the words);

  all in both languages where they have two.

## Related

[750](750-effector-agent.md), [750-EFF-0013](750-EFF-0013-development-and-maintenance-modes.md), [750-EFF-0015](750-EFF-0015-the-agents-mark-on-the-screen.md), [750-UPD-0020](750-UPD-0020-the-gateways-client-of-the-updater.md), [351](351-self-update.md), [800-APP-0108](../issues-done/800-APP-0108-the-links-lines-as-the-consoles.done).
