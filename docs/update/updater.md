# The updater

**Version:** 0.6 (2026-10-24): the release's loader staged into its slot (351-UPD-0010); 0.5 (2026-10-23): metadata past its expiry, the policy and `metadata` (351-UPD-0025); 0.4 (2026-10-22): a fallback runs its slot's own applications (351-UPD-0023, 351-KRN-0151); 0.3 (2026-10-22): a release's applications staged in its slot (351-UPD-0023); 0.2 (2026-10-20): a trial confirmed once `init` says it confirmed it (351-UPD-0021); 0.1 (2026-10-10) · **Track:** `UPD`, task [351-UPD-0007](../../issues-done/351-UPD-0007-updater-service.done) · **Constitution:** MC-9.2–9.4, MC-3.11, MC-11.11 · Russian: [updater_RU.md](updater_RU.md)

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
  - its expiry against the clock. A clock that cannot be read counts as expired: the updater does not take metadata it cannot show to be current (MC-9.4). What a device does past the expiry is [below](#metadata-past-its-expiry).
- **The version.** A channel whose version is not above the running one is refused (`older`). The running version is the one the channel kept in the running slot shows: that channel must verify with the release key and name, for this architecture, the manifest the bootloader verified and booted (its digest is in `BootInfo`). A slot without such a channel, such as one made by `boot_slots.py layout`, runs version 0, "not known".
- **The manifest** of this architecture: its SHA-256 must be the one the channel names, and its signature the **boot key**'s, the same key the bootloader checks.
- **Every file of the slot** the manifest lists, by size and SHA-256 after it is on the disk: `kernel.elf`, the services of `BOOT_FILES`, the loader for its processor (`EFI/BOOT/BOOTX64.EFI`, put in the slot as `BOOTX64.EFI`, 351-UPD-0010, [slots.md](slots.md)) and the applications, every other `*.elf` at the manifest's root (351-UPD-0023). The licences and data stay at the volume's root, shared by both slots ([slots.md](slots.md)), and are not fetched.

The public keys are built in: `$MIND_RELEASE_PUBLIC_KEY` and `$MIND_BOOT_PUBLIC_KEY` (files of 64 hex digits), or the public test keys (`updater/keys/release-test.pub`, `bootloader/keys/test.pub`). With a test key the updater says so at start.

## What it does

| Request (`idl/update.wit`) | What happens |
|---|---|
| `check` | The channel, checked as above. Returns what it offers and the running version. |
| `fetch` | `check`, then the newer release into the slot that did not boot. First it removes the slot's `CHANNEL`, `MANIFEST`, `MANIFEST.SIG`, and every file the new manifest does not list (the bootloader refuses a slot with a boot file its manifest does not list). Then it fetches every boot file, the loader and every application. An application the volume already holds as listed, in the running slot or at the root, is copied from there instead. A partial file is resumed where it ends, a cut connection is resumed with `Range`, and a file that is not as listed is fetched once more whole, then refused. Last come `MANIFEST`, `MANIFEST.SIG` and `CHANNEL`: a slot holds a channel only once all of it is there. |
| `apply` | Checks the other slot as the bootloader will: the manifest's signature, every boot file it lists, no boot file it does not list, and its loader as listed if it holds one (a slot staged before 351-UPD-0010 holds none). A slot that holds applications must hold every one the manifest lists and none it does not. A slot staged before 351-UPD-0023 holds none, and the root's serve it. The channel kept there must show a version above the running one, and must not have expired (`expired`). Then it writes the record that boots that slot on trial with 1 to 9 tries, falling back to the running slot, and asks `init` to restart the machine. |
| `rollback` | The same check of the other slot. The channel kept there must show a version not below the highest minimum the updater has seen (the running slot's channel, and the last check). Then it writes the record that boots that slot on trial, falling back to the running one, and asks `init` to restart. |
| `status` | The running slot, whether it is on trial, its version, the slot the newer record names and whether it is confirmed, the version staged in the other slot, and the last failure. |
| `metadata` (1.1) | The channel the updater holds, checked with the release key: the one it last read from the source, or else the one kept in the running slot. Its name, version, minimum and expiry, whether the expiry has passed, and whether it came from the source. |

The other slot is not written while the running slot is on trial and not yet confirmed on the disk: it is then the fallback. A request runs to its end before the next one is read.

**Confirming a trial.** On a trial boot the kernel restarts the machine at its deadline unless `init` confirmed the boot (351-KRN-0014).
- **It asks `init`.** Every second the updater asks `init` how the boot went (`init.wit` 1.4 `boot`, 000-KRN-0102; 351-UPD-0021). `init` answers once it has confirmed or refused the boot.
  - Confirmed: the updater writes the record that confirms the running slot at once, the same slot and fallback, confirmed (`[UPDATER] SLOT … CONFIRMED ON THE DISK: INIT CONFIRMED THE TRIAL BOOT`).
  - Refused: it writes nothing and does nothing automatic. The kernel restarts the machine into the other slot, which stays untouched.
- **When `init` cannot say** (an `init` before 1.4), the updater waits until 5 s past the kernel's deadline, on the same clock. If the machine is still running then, the boot was confirmed, and it writes the same record.
- Until the trial is settled it does nothing automatic.

**Records.** Each record goes into the file that does not hold the newer valid record, one past its sequence ([slots.md](slots.md)). It is written whole and in place, then read back. A sequence that could not count two further (a trial and its failure) is not written. `updater/src/plan.rs` chooses the record; its host tests run the bootloader's own encoding (`tests/updater_host.rs`).

## Metadata past its expiry

A device may go longer than its channel's expiry without a current one: offline, or with a source that is not answering. The policy (MC-9.4; 351-UPD-0025):
- **The system runs on.** It keeps its confirmed slot. An expiry stops updates, never the system.
- **No update until a current channel comes.**
  - `check` and `fetch` refuse an expired channel (`expired`).
  - `apply` refuses a staged release whose kept channel has expired.
  - The next automatic run, or a request, tries the source again.
- **`rollback` stays possible.** It needs no network, and the minimum still holds: the version not below the highest minimum the updater has seen.
- **A clock that cannot be read** counts as expired.
- **It says so.**
  - In the log: `[UPDATER] METADATA: CHANNEL … VERSION …, MINIMUM …, EXPIRES …, KEPT IN SLOT A` at start, and `… FROM THE SOURCE` after each check that reads another channel. Either line ends `; EXPIRED: FETCH AND APPLY WAIT FOR A CURRENT CHANNEL` once the expiry has passed.
  - To a client: `metadata` (`update.wit` 1.1) gives the same.
- **Freezes.** A source that keeps serving the same unexpired channel holds a device on it until it expires. A shorter bound is the timestamp key ([351-UPD-0028](../../issues/351-UPD-0028-a-short-lived-timestamp-key.md)).

## update.txt

A file at the root of the boot volume. Nothing in the running system writes it: it is written with the image, or on another machine.

```
source https://updates.example.org/mind   # or a directory: usb:releases on a mounted volume, data/link/rel on the boot volume
pin 3c5f…                                 # https: the SHA-256 of the server's public key; without it, tlsroots.pem
channel stable
automatic apply                           # none (the default), check, fetch or apply
every 21600                               # seconds between automatic runs; 0 (the default): once at start
tries 3                                   # tries of a trial boot for an automatic apply (1 to 9)
```

- **A directory source** is checked as the HTTPS one: the source only supplies bytes, and every check above is the updater's. A file whose size is not the manifest's is refused before it is copied, and the next run takes it again (351-KRN-0141).
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
  - The image's slot A holds release 5's applications, and they run from there (`[LOADER] APPLICATIONS FROM MIND/A/`).
  - Version 6 differs from 5 in one application, `uptime.elf`. Only that one is fetched; the others are copied from slot A, and slot B then holds every application as the manifest lists it. The trial runs them from slot B.
  - Booted back in slot A, from the record a fallback or a rollback leaves (written by the host), release 5 runs its own applications from slot A, its `uptime` among them.
  - It is checked and booted on trial through `init`'s restart. The bootloader loads release 6's manifest.
  - As soon as `init` confirmed the trial, the updater confirms slot B on the disk, well before the kernel's deadline, then finds nothing newer.
  - The records then hold exactly the three writes of an update: the trial, the bootloader's count, the confirmation. `fsck.fat` finds the volume consistent.
  - The log names the channel held (351-UPD-0025): slot A's at start, current, then the source's, expired.
  - Booted back in slot A with the clock 60 days on, past the expiry of the channel kept there, and no network, the updater says that channel has expired, and the system runs on: `uptime` runs.
- **The `linkupdate` suite** (351-KRN-0141; x86, aarch64) refuses an expired channel from a directory, as over HTTPS, with no boot record written.
- **Host tests** (`tests/updater_host.rs`) cover each record the updater writes, followed through the bootloader's choice:
  - an update's trial;
  - its confirmation;
  - a failed trial, which leaves the fallback running;
  - a rollback;
  - no valid record;
  - a sequence at its end.

  They also cover how a manifest's files split into boot files, applications and the root's shared files, and when metadata counts as expired: from the second of its expiry on, or always on a clock that cannot be read.

## Not provided yet

- **A caller of `idl/update.wit`.** No program holds a client of the updater: the shell's `update` command is the tools track's (351-APP-0029, numbered on its branch from [requests-APP.md](../../issues/requests-APP.md)), and the slot for its client is asked of the kernel track ([requests-KRN.md](../../issues/requests-KRN.md)). So `check`, `fetch`, `apply`, `rollback`, `status` and `metadata` run only through the automatic policy in the tests. `rollback`, `status` and `metadata` have not run on the platform, nor `apply`'s refusal of an expired staged release: the automatic run checks the source first.
- **The directory source's other refusals, in QEMU.** The `linkupdate` suite (351-KRN-0141) fetches a release from `data/link/rel/`, boots it on trial and confirms it, and refuses an expired channel and one with a changed signature. An older channel, a changed boot file and a resumed file go through the same `stage()` as over HTTPS, where the `update` suite shows each, but none of them has run from a directory.
- **SSH** (351-NET-0004).
- **Rollback protection** beyond the version check: a fallback is booted whatever its version (351-UPD-0009, 0011).
- **Key rotation and revocation** (351-UPD-0009). The bootloader is not updated (351-UPD-0010).
- **Slot images by default.** `make_usb_image.py --slots` makes an image the updater can update ([write-disk.md](../write-disk.md), 351-UPD-0016); without the option an image boots from its root and has no slot to fill.
- **The clock.** Expiry is only as good as the system clock, and the updater has no second source of time.
