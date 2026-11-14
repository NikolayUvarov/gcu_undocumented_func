# 801-LNK-0046 — Each machine's settings kept by `build_and_write.sh`, and put on any new disk

**Type:** tools (`scripts/build_and_write.sh`) · **Owner:** `LNK` · **Priority:** P1 · **Status:** in progress (2026-11-14) · **Blocked by:** — · **Main task:** [801](801-the-bench-stays-manageable.md) · **Constitution:** MC-4, MC-12.3

## Problem

The maintainer (2026-11-14): «Каждая перезапись должна сохранять локально настройки». The MacBook Pro's USB disk is to be replaced by a stick for the testing period, so the settings must reach a new disk too.

`--keep-settings`, `--keep-wifi` and `--keep-effector` save into one place, `~/.config/mind/settings` and its kin. That place holds whatever disk was read last:
- a write that skips the old disk takes another machine's files;
- in October the DEXP got the MacBook Pro's agent this way.

**Found on the bench, 2026-11-14: the MacBook Pro and the DEXP carry the same device key.** Both logs say `[KEYSTORE] DEVICE KEY READY: MIND 5097010F`.
- The Mac reads it from `system/keystore/device.key`, on the disk: it has no TPM.
- The DEXP reads it from `system/keystore/device.sealed`, sealed by its TPM (release 202610302359).
- The likely path: `--keep-wifi`'s one store carried the Mac's `device.key` onto a DEXP disk that had no key. The DEXP's keystore then took it, sealed it in its TPM and removed the plain copy (`keystore/src/main.rs`, `sealed_seed`).
- One key on two machines is one identity on both: the device certificate, the SSH host key, and the key the remembered Wi-Fi networks are wrapped under.
- **A second gap:** `--keep-wifi` kept `wifi.kept`, `device.key` and `wifi.country`, but not `device.sealed` or `device.locked`. So a TPM machine's key, or one sealed under the owner's passphrase, was lost at each rewrite, and with it the networks wrapped under it. 801's DEXP line `REMEMBERED WI-FI NETWORKS DO NOT OPEN UNDER THIS DEVICE KEY: FORGOTTEN` fits that.

## Plan

1. **Kept per machine:** `~/.config/mind/machines/<id>/`, where `<id>` is the agent's ID in `system/effector/`, or the device key's public fingerprint when there is no agent. Never a secret in the name.
2. **Before a write:** the old disk is read, and its machine's directory is updated.
3. **After the write:** the settings of the machine named by `--machine <id>`, or by the old disk just read, are put on the new disk. A different device than the old one is allowed.
4. **No choice by default:** a write with neither an old disk nor `--machine` is refused. The script never falls back to "the last one".
5. **`--list-machines`** names the kept machines, by ID, host name and date, and no secret.

## Acceptance criteria

- [x] **Tests on temporary directories:**
  - two machines kept side by side;
  - a write onto a new device from one machine's directory;
  - a write with no machine refused.
- [ ] **On the bench:** the MacBook Pro's settings put on a new stick, its agent back as the same endpoint, its Wi-Fi joined.

## Progress

- **2026-11-14, the host part** (`scripts/build_and_write.sh`, branch `claude/801-LNK-0046-settings-per-machine`):
  - **Kept per machine** in `~/.config/mind/machines/<id>/`, 700, files 600: `keystore/`, `netpolicy/`, `effector/`, `settings/`, the `.prev` copies of 800-LNK-0036 beside them, and `machine.txt` (ID, host name, the agent's machine print, the date saved, the old disk's commit).
  - **`<id>`** is the agent's ID from `system/effector/agent_id`, else the owner's `data/effector/agent_id`. Without an agent it is `host-<name>`, from `data/machine/name` or `effector.conf`'s `hostname`.
    - The device key's fingerprint is not used. `system/keystore/device.key` holds only the secret seed (`keystore/src/stored.rs`), and its public half exists only in the key service's answers. A fingerprint would need the seed, so the plan's fallback is the name instead.
    - A disk that names no machine (the default name `mindcore`, no agent) is not read.
  - **Before the write,** the old disk's volume is mounted once. The machine it names is found, and each `--keep-*` part is saved into that machine's directory by the functions of 800-LNK-0023, 0024 and 0036.
  - **After the write,** the machine named by `--machine <id>`, else by the old disk just read, is put on the disk.
    - With neither, the script stops before the writer opens. The message names `--list-machines`.
    - The same happens for `--machine` with nothing kept, and for a machine's directory that cannot be written.
    - With `--machine`, a disk that holds another machine is not read, and that machine's files stay as they are.
  - **`--old-disk <n>`:** the old disk is read first as disk `<n>`. The script then asks for it to be taken out and the new one inserted as the disk to be written. It waits until a device other than the old one is there, telling them apart by Windows' unique ID, serial number and size. Enter goes on as it is.
  - **`--list-machines`** shows the ID, the host name, the date saved, the parts kept and the device key's public label. It stops before anything is fetched, built or written.
  - **The device key in every form:** `--keep-wifi` now keeps `device.sealed` and `device.locked` beside `device.key`. A sealed key opens only on its own machine's TPM or passphrase. So a TPM machine's key survives a rewrite of its own disk; on another machine keystore refuses it and makes a new one.
  - **One device key on one machine:**
    - `machine.txt` records the key's public label, `MIND` and 8 hex digits, as keystore logs it. It is computed from `device.key` with `openssl`, the seed passing through a pipe only. For a sealed key, it is read from the old disk's newest `BOOTNNNN.LOG` on its `MIND LOG` volume.
    - A write whose machine's kept key another kept machine holds too stops before the writer opens. The message names both machines and the label, never the key.
    - A sealed key whose label is in no log is said, and not refused.
    - `--new-device-key` resolves it for the machine written. Its kept key and networks are moved aside into `keystore/replaced-<date>/`, never removed, and nothing of the old key goes on the disk. keystore then makes a new key there, and the networks are joined again.
    - For the bench: whichever of the Mac and the DEXP is written second is stopped. The maintainer gives `--new-device-key` to the one that is to get a new key.
  - **The one store of before** (`~/.config/mind/settings`, `effector`, `keystore`, `netpolicy`) is never read into a machine. The script says that it is left untouched and unused (no compatibility code, AGENTS.md section 3).
  - **Docs:** the script's header, and `docs/write-disk.md` with its Russian version.
  - **Tests:**
    - `tests/build_and_write_machines_test.sh` takes the functions from the script by name. It runs them on plain directories with dummy files; `sudo mount` and `umount` are stand-ins that link the mount point. 61 checks, all passing:
      - two machines kept side by side;
      - one read again while the other stays as it was;
      - a write onto a new device from one machine's directory;
      - the old disk read, then another device written;
      - the refusals: no machine, a nameless disk, a machine not kept, a directory that cannot be written;
      - a disk of another machine left unread with `--machine`;
      - `--list-machines` with no secret printed;
      - the one store of before left as it was;
      - `--old-disk`'s wait, on a played-back sequence of disk states;
      - the device key's label against RFC 8032's public test vectors 1 and 2, and a damaged key with none;
      - a sealed key's label from the newest boot log;
      - the bench's case (a `device.key` on one machine, the same key sealed on another) refused both ways, both named, no key material in the output;
      - `--new-device-key` setting the key aside and putting none of it on the disk;
      - a TPM machine's `device.sealed` back on its own new disk;
      - an unknown sealed label said, not refused.
    - `tests/build_and_write_args_test.py` runs the script itself as far as its arguments: the refusals of `--machine`, `--old-disk` and `--new-device-key`, and `--list-machines` on a temporary `HOME`.
    - `scripts/host_tests.sh` runs both.
  - **Not done here:**
    - No disk was written. The prompts and Windows' disk states (`Get-Disk`, the `MIND LOG` volume's letter, the drive letters) were not run against a real device.
    - The new PowerShell was checked only by PowerShell's parser, and its identity formula on a stand-in object.
    - The labels of the bench's own keys were not computed here: no real key was read.

## Progress (2026-11-14, later): the pull before the options

**The maintainer:** «Он должен вызывать git pull изнутри себя и не требовать запуска предварительного git pull!!!»

**The problem.** The script read its options before it pulled `fast-test`. A copy older than an option (the maintainer's clone was on `fast-test` 44a1a4f1 of 2026-11-06) took `--old-disk` for the disk number and stopped with `Disk number must be digits`, before the pull that would have brought the new script.

**The fix.**
- **The pull runs first:** the fetch, the checkout and `git pull --ff-only`, then the new script with the same arguments (800-LNK-0025). Only then are the options read, so they are always the current script's.
- **Looked for beforehand:** `--commit <id>`. `--image`, `--list` and `--list-machines` fetch nothing, as before.
- **An unknown option** is now named (`unknown option …`), not taken for a disk number.

**What this does not fix.** A clone that holds an older copy still reads options first, once. It takes this version on its next run with options it knows, or with one `git pull` there.

**Tested:**
- `tests/build_and_write_args_test.py`: 13 passed. The 2 new tests:
  - a clone of a temporary repository, given an option it does not know, pulls and runs the newer script with the same arguments;
  - an unknown option is refused by name.
- The keep and machines tests: passed.

## Progress (2026-11-14, later still): the old disk may stay in (taken back, below)

**The maintainer, on the first real run** (MacBook Pro, `--old-disk 5 2`): «Почему это я должен вынимать диск?!» The script had read the old disk and kept the Mac's settings. Then it waited for disk 5 to be taken out, and showed nothing while it waited.

**Why it waited:** after the write, the steps that put the settings and flags on the new disk took the first volume MIND CORE that Windows listed, and checked only that one for this build's MANIFEST. With the old disk still in, that could be the old volume, so the new one would not be found.

**The fix:**
- **The steps after the write** pick, among all volumes MIND CORE, the one holding this build's MANIFEST (`PS_FLAG`, `PS_LETTER`, `PS_LINK`).
- **The swap** waits only until disk N is there and is another device than the old one. The old one may stay in.
- **While it waits,** it shows the state every 15 s.

**Tested:**
- `tests/build_and_write_machines_test.sh`: all passed, with the new message.
- The args and keep tests: passed.
- PowerShell's parser: no errors in the four snippets.
- **The new lookup run on this PC, read-only,** against the real old disk 5 (volume `P:`): it named `P` for that disk's `MIND\A\MANIFEST` hash, and nothing for a wrong one.

## Progress (2026-11-14, last): the old disk is taken out after all; the MBR signature stays one

**What the first real run showed.** With both disks in, Windows took the new disk 2 offline right after the write: `OfflineReason Collision`. Every disk MIND Core writes has the same MBR signature, `BE1AFDFA`, and so did the old disk 5. The steps after the write then found no volume, and nothing was put on the new disk: no settings, no firmware, no pictures.

**The maintainer's decision:**
- «пусть MBR будет одинаковая, это правильно»: the MacBook Pro boots the disk by that signature;
- «Давай вариант с выниманием первого диска вернем»: the old disk is taken out again.

**Now:**
- **The swap waits for the old device to be out** and disk N to be there. It says why, and shows its state every 15 s.
- **The steps after the write** still pick the volume holding this build's MANIFEST among all volumes MIND CORE.
- **A different signature per disk** was considered and dropped.

## Related

[801](801-the-bench-stays-manageable.md), [800-LNK-0023](800-LNK-0023-keep-the-effector-agent-across-a-write.md), [800-LNK-0024](800-LNK-0024-keep-the-owners-settings-across-a-write.md).
