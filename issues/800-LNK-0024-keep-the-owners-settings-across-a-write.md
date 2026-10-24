# 800-LNK-0024 — `build_and_write.sh --keep-settings`: everything the owner set kept across a write

**Type:** tool · **Owner:** `LNK` · **Priority:** P1 · **Status:** in progress (2026-10-24) · **Blocked by:** — · **Main task:** [800](800-mind-link.md) · **Constitution:** MC-12.6

## Problem

The maintainer (2026-10-24): each new stick loses the machine's settings, such as the time zone and the desktop's picture, so the machine has to be set up again after every write. A flag should keep all of them.

What the boot volume holds that the owner sets (from the programs' paths in `main`):

| Where | What |
|---|---|
| `data/clock.conf` | the time zone (the shell's `clockzone`) |
| `data/wm.conf`, and its picture under `data/` | the desktop (`wm`'s background, its clock and place) |
| `data/memory.conf` | memory for named programs (262-KRN-0160) |
| `data/services.txt` | `init`'s service configuration (173-KRN-0035) |
| `data/mode.conf` | the modes (750-KRN-0143) |
| `data/apps/` | applications installed through the link |
| `data/media/`, `data/inbox/` and others | the owner's files |
| `system/keystore/`, `system/netpolicy/` | Wi-Fi keys and the changed network policy (`--keep-wifi`) |
| `data/effector/`, `system/effector/` | the Effector agent (`--keep-effector`, 800-LNK-0023) |
| `system/effector_gw/` | the Effector gateway's state |

## Plan and what was done

- **`--keep-settings`** turns on `--keep-wifi` and `--keep-effector`, each with its own care: the network policy is merged with the new image's, and the agent's files are kept whole. It also keeps the rest of `data/` and `system/`.
  - **Saved before the write** into `~/.config/mind/settings/{data,system}`, 700 and 600, whole or not at all, from the old disk. The disk is asked for and waited on, as for 800-LNK-0023.
  - **Put back first** after the write, each file read back. Then what the script puts itself goes on top: proprietary firmware, pictures, `--mode=` and the other options' files, Wi-Fi, the agent. So an option given now still wins over a kept file.
  - **Not kept:**
    - `data/firmware` and `data/wallpapers`, which the script copies;
    - the test flags `wifitest.txt`, `wifi-ofdm.txt` and `wifijoin.txt`, which its options decide;
    - `data/link`: the link's scratch and old releases, which the updater should not see again.
- **Docs:** the script's header, and `docs/write-disk.md` with its Russian version.
- **Checked on the host,** on directories:
  - kept: 9 files, among them `apps/`, `media/` and a directory with a space;
  - skipped: everything listed above;
  - put back over the new image's `mode.conf` and `services.txt`, and read back.

## Acceptance criteria

- [x] The helpers keep the right files, skip the script's own, and put them back read back (host check above).
- [ ] On the MacBook Pro, after a write with `--keep-settings`, the time zone and the desktop are as before, Wi-Fi joins, and the Effector agent comes up as the same endpoint.

The saved files may hold keys and the agent's token: never printed, logged or committed.

## Progress

- **2026-10-24, the first write with `--keep-settings`** (11:08 UTC, the new script).
  - **What it kept:** the time zone (`TIME ZONE UTC+03:00 FROM data/clock.conf` on the new stick), the agent's `policy` and `agent_id`.
  - **What it could not keep:** `effector.conf`, which was not on the old stick. The stick had been pulled from the running Mac, and `vfs_server` had not yet written `start.msh`'s `rm` and `mv` to it. A reboot after the next `start.msh` showed the file kept.
  - **What followed:** KRN is asked to flush after a rename or a remove. EFF is asked to end `start.msh` with `sync`. `docs/write-disk.md` (EN, RU) says to shut down or reboot before taking the disk out.

## Related

[800-LNK-0023](800-LNK-0023-keep-the-effector-agent-across-a-write.md), 800-LNK-0014.
