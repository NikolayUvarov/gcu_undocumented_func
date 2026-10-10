# The updater

**Version:** 0.1 (2026-10-10) · **Track:** `UPD`, task [351-UPD-0007](../../issues/351-UPD-0007-updater-service.md) · **Constitution:** MC-9.2–9.4, MC-3.11, MC-11.11 · Russian: [updater_RU.md](updater_RU.md)

`updater` is the boot service that turns a published release ([publishing.md](publishing.md)) into a staged, verified slot ([slots.md](slots.md)), and does nothing more. It holds:
- its own endpoint;
- the clock;
- a client of `vfs_server` badged for the update zone: the slot that did not boot and the two boot records;
- the parser service;
- `init`'s client badged for `reboot`;
- the log;
- the flow grant the network policy gives `updater`;
- the TLS client.

It cannot write anything else, and the manifest it reads grants nothing (MC-3.11).

## What it checks

- **The channel.** The parser service reads its fields, and the updater takes them only if they encode again to exactly the bytes it fetched (MC-11.11). Then it checks:
  - the signature over the first line, with the **release key** built into it;
  - the channel's name;
  - its expiry against the clock. A clock that cannot be read counts as expired: the updater does not take metadata it cannot show to be current (MC-9.4).
- **The version.** A channel whose version is not above the running one is refused (`older`). The running version is the one the channel kept in the running slot shows: that channel must verify with the release key and name, for this architecture, the manifest the bootloader verified and booted (its digest is in `BootInfo`). A slot without such a channel, such as one made by `boot_slots.py layout`, runs version 0, "not known".
- **The manifest** of this architecture: its SHA-256 must be the one the channel names, and its signature the **boot key**'s, the same key the bootloader checks.
- **Every boot file** the manifest lists: `kernel.elf` and the services of `BOOT_FILES`, by size and SHA-256 after it is on the disk. Applications stay at the volume's root, shared by both slots ([slots.md](slots.md)), and are not fetched.

The public keys are built in: `$MIND_RELEASE_PUBLIC_KEY` and `$MIND_BOOT_PUBLIC_KEY` (files of 64 hex digits), or the public test keys (`updater/keys/release-test.pub`, `bootloader/keys/test.pub`). With a test key the updater says so at start.

## What it does

| Request (`idl/update.wit`) | What happens |
|---|---|
| `check` | The channel, checked as above. Returns what it offers and the running version. |
| `fetch` | `check`, then the newer release into the slot that did not boot. First it removes the slot's `CHANNEL`, `MANIFEST`, `MANIFEST.SIG`, and every file the new manifest does not list (the bootloader refuses a slot with a boot file its manifest does not list). Then it fetches every boot file: a partial one is resumed where it ends, a cut connection is resumed with `Range`, and a file that is not as listed is fetched once more whole, then refused. Last come `MANIFEST`, `MANIFEST.SIG` and `CHANNEL`: a slot holds a channel only once all of it is there. |
| `apply` | Checks the other slot as the bootloader will: the manifest's signature, every boot file it lists, no boot file it does not list. The channel kept there must show a version above the running one. Then it writes the record that boots that slot on trial with 1 to 9 tries, falling back to the running slot, and asks `init` to restart the machine. |
| `rollback` | The same check of the other slot. The channel kept there must show a version not below the highest minimum the updater has seen (the running slot's channel, and the last check). Then it writes the record that boots that slot on trial, falling back to the running one, and asks `init` to restart. |
| `status` | The running slot, whether it is on trial, its version, the slot the newer record names and whether it is confirmed, the version staged in the other slot, and the last failure. |

The other slot is not written while the running slot is on trial and not yet confirmed on the disk: it is then the fallback. A request runs to its end before the next one is read.

**Confirming a trial.** On a trial boot the kernel restarts the machine at its deadline unless `init` confirmed the boot (351-KRN-0014). The updater waits until 5 s past that deadline, on the same clock. If the machine is still running then, the boot was confirmed. It then writes the record that confirms the running slot: the same slot and fallback, confirmed. Until then it does nothing automatic.

**Records.** Each record goes into the file that does not hold the newer valid record, one past its sequence ([slots.md](slots.md)). It is written whole and in place, then read back. A sequence that could not count two further (a trial and its failure) is not written. `updater/src/plan.rs` chooses the record; its host tests run the bootloader's own encoding (`tests/updater_host.rs`).

## update.txt

A file at the root of the boot volume. Nothing in the running system writes it: it is written with the image, or on another machine.

```
source https://updates.example.org/mind   # or a directory on a mounted volume: usb:releases
pin 3c5f…                                 # https: the SHA-256 of the server's public key; without it, tlsroots.pem
channel stable
automatic apply                           # none (the default), check, fetch or apply
every 21600                               # seconds between automatic runs; 0 (the default): once at start
tries 3                                   # tries of a trial boot for an automatic apply (1 to 9)
```

- **Without the file** the updater has no source and acts only when asked.
- **The automatic run** starts once the trial (if any) is confirmed. When the source does not answer, it is tried again after 30 s.
- **The source** may be an HTTPS server or a directory with the layout of `scripts/release.py`. Both go through the same checks.
- **HTTPS** runs over the flow grant the network policy gives `updater`, so `netpolicy.txt` must name the server.

## Tested

- **The `update` check** (part of the `updater` suite; x86, aarch64). The image holds release 5 in slot A, confirmed, with its channel kept there. `update.txt` names the test server (`scripts/serve_release.py`) over HTTPS, trusted by its pinned key, with `automatic apply` every 20 s.
  - Each of these is refused, and the boot records stay as they were (the final sequence numbers show it):
    - an expired channel;
    - a channel with a bad signature;
    - a channel naming an older version;
    - version 6 with one boot file changed on the server.
  - Then version 6 as published is fetched into slot B. Its largest boot file is cut midway and resumed with `Range`.
  - It is checked and booted on trial through `init`'s restart. The bootloader loads release 6's manifest.
  - Past the kernel's deadline the updater confirms slot B on the disk, then finds nothing newer.
  - The records then hold exactly the three writes of an update: the trial, the bootloader's count, the confirmation. `fsck.fat` finds the volume consistent.
- **Host tests** (`tests/updater_host.rs`) cover each record the updater writes, followed through the bootloader's choice:
  - an update's trial;
  - its confirmation;
  - a failed trial, which leaves the fallback running;
  - a rollback;
  - no valid record;
  - a sequence at its end.

## Not provided yet

- **A caller of `idl/update.wit`.** No program holds a client of the updater: the shell's `update` command is the tools track's (351-APP-0029, numbered on its branch from [requests-APP.md](../../issues/requests-APP.md)), and the slot for its client is asked of the kernel track ([requests-KRN.md](../../issues/requests-KRN.md)). So `check`, `fetch`, `apply`, `rollback` and `status` run only through the automatic policy in the tests, and `rollback` and `status` have not run on the platform.
- **A directory source** is built but not tested in QEMU.
- **SSH** (351-NET-0004).
- **A knowing confirmation.** The updater infers the kernel's confirmation from the deadline. Asking `init` would take seconds instead of the 2 minutes the deadline takes ([requests-KRN.md](../../issues/requests-KRN.md)).
- **Rollback protection** beyond the version check: a fallback is booted whatever its version (351-UPD-0009, 0011).
- **Key rotation and revocation** (351-UPD-0009). The bootloader is not updated (351-UPD-0010).
- **Applications in the slots** ([slots.md](slots.md)).
- **Images built with slots** ([351-UPD-0016](../../issues/351-UPD-0016-images-with-slots.md)).
- **The clock.** Expiry is only as good as the system clock, and the updater has no second source of time.
