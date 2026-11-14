# The updater

**Version:** 0.16 (2026-11-14): the cable's `data/link/rel/` a source always, whatever `update.txt` says (801-UPD-0039; 0.15 is 801-UPD-0037's); 0.14 (2026-11-13): the boot keys are the key file's; the slot's key files staged and checked as the bootloader checks them, and their version raised in the records at a confirmation (351-UPD-0029); 0.13 (2026-11-13): recovery orders, the one way below the floor (351-UPD-0030); 0.12 (2026-11-13): the channel's timestamp read first, its expiry the channel's at most (351-UPD-0028); 0.11 (2026-11-13): the key file taken only as the next version, kept in `system/updater`; a channel or slot only a revoked key signed refused (351-UPD-0027); 0.10 (2026-11-10): `apply` writes a damaged staged slot again from the block store, and `kept` lists the releases it keeps (351-UPD-0034, 0035, `update.wit` 1.5); 0.9 (2026-11-08): releases kept in the block store, and a rollback writes a damaged slot again from there (351-UPD-0033, `update.wit` 1.4); 0.8 (2026-10-25): the floor in the boot records, format 2; records of format 1 not read (351-UPD-0024); 0.7: the version floor raised to the channel's minimum; 0.6 (2026-10-24): the release's loader staged into its slot (351-UPD-0010); 0.5 (2026-10-23): metadata past its expiry, the policy and `metadata` (351-UPD-0025); 0.4 (2026-10-22): a fallback runs its slot's own applications (351-UPD-0023, 351-KRN-0151); 0.3 (2026-10-22): a release's applications staged in its slot (351-UPD-0023); 0.2 (2026-10-20): a trial confirmed once `init` says it confirmed it (351-UPD-0021); 0.1 (2026-10-10) · **Track:** `UPD`, task [351-UPD-0007](../../issues-done/351-UPD-0007-updater-service.done) · **Constitution:** MC-9.2–9.4, MC-3.11, MC-11.11 · Russian: [updater_RU.md](updater_RU.md)

`updater` is the boot service that turns a published release ([publishing.md](publishing.md)) into a staged, verified slot ([slots.md](slots.md)), and does nothing more. It holds:
- its own endpoint;
- the clock;
- a client of `vfs_server` badged for the update zone: the slot that did not boot and the two boot records; and for its private directory, `system/updater`, which no other badge opens (351-UPD-0027);
- the parser service;
- `init`'s client badged for `reboot`;
- the log;
- the flow grant the network policy gives `updater`;
- the TLS client;
- the block store's client badged `OWNER_UPDATER` with get, put and publish, when the store runs (351-KRN-0218).

It cannot write anything else, and the manifest it reads grants nothing (MC-3.11).

## What it checks

- **The key file** (`keys/current`, 351-UPD-0026, [publishing.md](publishing.md)), first. The parser service reads it, and the updater takes it only as the channel's fields below. It holds one key file, the highest version it took (351-UPD-0027), and takes another only as the **next version** (`release::step`): signed by at least the threshold of the held file's `root` role, or of the root keys built in for version 1, and of the new file's own `root` role; each root key counted once and a line of any other key passed over. A source several versions ahead gives each one between as `keys/<version>`, each taken in turn. Then the last one's expiry against the clock. Before any channel is read:
  - a file the root before it did not sign to its threshold is refused: `[UPDATER] KEY FILE VERSION 2 REFUSED (keys/current): FEWER THAN 2 OF VERSION 1'S ROOT KEYS SIGNED IT`;
  - a lower version is refused (`older`), since it would bring back a key a later one revoked: `[UPDATER] KEY FILE VERSION 1 REFUSED (keys/current): BELOW VERSION 3, TAKEN BEFORE: IT WOULD BRING REVOKED KEYS BACK`;
  - the same version is taken only as the same file.

  Each version taken is kept in `system/updater/keys`, written whole beside it (`keys.new`) and then put in its place: `[UPDATER] KEY FILE VERSION 3 TAKEN (keys/current), THE ROOT ROTATED, KEPT IN system/updater/keys`. At start the updater reads it back, the higher version of the two files that its own root signed (`[UPDATER] KEY FILE VERSION 3 FROM system/updater/keys, TAKEN BEFORE`). Without one, it takes a slot's `KEYS` when that is the next version: an image as written holds version 1 there.
- **The timestamp** (`timestamps/<channel>`, 351-UPD-0028), next. The parser service reads it as the channel below. It is refused, before the channel is read and with nothing written, when:
  - fewer than the threshold of the timestamp keys the key file names signed it: `[UPDATER] TIMESTAMP VERSION 9 REFUSED: FEWER THAN 1 OF THE TIMESTAMP KEYS OF KEY FILE VERSION 1 SIGNED IT` (`signature`);
  - it is another channel's (`malformed`);
  - its version is below the highest the updater took before, kept in `system/updater/timestamp` across restarts (`older`);
  - it has expired, or the clock cannot be read (`expired`).

  Then the channel file must be the one it names by SHA-256 (`[UPDATER] TIMESTAMP VERSION 9 NAMES ANOTHER FILE THAN channels/stable`, `digest`). The channel's expiry counts as the shorter of its own and the timestamp's: `metadata` and the log show that one, and `apply` waits while it has passed. A rollback, and a release written again from the block store, need no timestamp.
- **The channel.** The parser service reads its fields, and the updater takes them only if they encode again to exactly the bytes it fetched (MC-11.11). Then it checks:
  - the signature lines over the first line: at least the release role's threshold of the **release keys** the key file names;
  - the channel's name;
  - its expiry against the clock. A clock that cannot be read counts as expired: the updater does not take metadata it cannot show to be current (MC-9.4). What a device does past the expiry is [below](#metadata-past-its-expiry).
- **The version.** A channel whose version is not above the running one is refused (`older`). The running version is the one the channel kept in the running slot shows: the channel must verify with the release keys of the key file the updater holds, and name, for this architecture, the manifest the bootloader verified and booted (its digest is in `BootInfo`). A slot without such a channel, such as one made by `boot_slots.py layout`, runs version 0, "not known". So does a slot whose channel only a key the held file no longer names signed (351-UPD-0027), and it is no fallback: `[UPDATER] SLOT A: ITS CHANNEL IS SIGNED BY FEWER THAN 1 OF THE RELEASE KEYS OF KEY FILE VERSION 3: NOT A FALLBACK`.
- **The manifest** of this architecture: its SHA-256 must be the one the channel names, and its signature by a key of the `boot` role of the key file the updater trusts (351-UPD-0029), a role of threshold 1: a manifest carries one signature. Otherwise `[UPDATER] FETCH: VERSION 7: ITS MANIFEST IS SIGNED BY NO BOOT KEY OF KEY FILE VERSION 2`.
- **Every file of the slot** the manifest lists, by size and SHA-256 after it is on the disk: `kernel.elf`, the services of `BOOT_FILES`, the loader for its processor (`EFI/BOOT/BOOTX64.EFI`, put in the slot as `BOOTX64.EFI`, 351-UPD-0010, [slots.md](slots.md)) and the applications, every other `*.elf` at the manifest's root (351-UPD-0023). The licences and data stay at the volume's root, shared by both slots ([slots.md](slots.md)), and are not fetched.

The public keys built in are the root set's alone: `$MIND_ROOT_KEYS` (`threshold T`, then a public key a line, as `release.py keys root` prints it), or the public test set (`updater/keys/root-test.pub`, three keys of which two sign); the bootloader is built with the same set (351-UPD-0029). No other key is built in: the key file names the boot, release, timestamp and recovery keys. With the test set the updater says so at start (`[UPDATER] KEYS: THE TEST ROOT KEYS`).

**The slot's key files** (351-UPD-0029). With the manifest and its signature the updater writes into the slot the key file it trusts, `KEYS`, and before it the key files it took that changed the root, as `KEYS.1`, `KEYS.2`, …: the bootloader follows the root built in through them. It keeps those in `system/updater/roots/<version>` as it takes them (`[UPDATER] KEY FILE VERSION 3 TAKEN (keys/current), THE ROOT ROTATED, KEPT IN system/updater/keys AND system/updater/roots`). Before an apply, and before a recovery order, it checks the other slot's key files as the bootloader will (`bootloader/src/keychain.rs`, the parser reading each file): under the root built in, its version not below the one the records name, and its manifest signed by a boot key they name. A slot below that version whose manifest a boot key of the trusted file signed gets the trusted key files first (`[UPDATER] SLOT A: KEY FILE VERSION 2 WRITTEN THERE, A BOOT KEY OF WHICH SIGNED ITS MANIFEST`).

**The records' key file version** (351-UPD-0029, [slots.md](slots.md)). The confirmation of a slot carries the version of that slot's key file into the records, never lower than theirs (`[UPDATER] KEY FILE VERSION 2 IN THE RECORDS: A SLOT WITH A LOWER ONE IS NOT BOOTED (MC-9.6)`). From then the bootloader refuses a slot whose key file is below it. When the version rises, the other slot first gets the trusted key files if a boot key they name signed its manifest, and stays a fallback; otherwise the updater says it is none: `[UPDATER] SLOT A: SIGNED BY NO BOOT KEY OF KEY FILE VERSION 2: NOT A FALLBACK FROM NOW ON`.

## What it does

| Request (`idl/update.wit`) | What happens |
|---|---|
| `check` | The channel, checked as above. Returns what it offers and the running version. |
| `fetch` | `check`, then the newer release into the slot that did not boot. First it removes the slot's `CHANNEL`, `KEYS`, `MANIFEST`, `MANIFEST.SIG`, and every file the new manifest does not list (the bootloader refuses a slot with a boot file its manifest does not list). Then it fetches every boot file, the loader and every application. An application the volume already holds as listed, in the running slot or at the root, is copied from there instead. A partial file is resumed where it ends, a cut connection is resumed with `Range`, and a file that is not as listed is fetched once more whole, then refused. Last come `MANIFEST`, `MANIFEST.SIG`, the key file as `KEYS` and `CHANNEL`: a slot holds a channel only once all of it is there. |
| `apply` | Checks the other slot as the bootloader will: the manifest's signature, every boot file it lists, no boot file it does not list, and its loader as listed if it holds one (a slot staged before 351-UPD-0010 holds none). A slot that holds applications must hold every one the manifest lists and none it does not. A slot staged before 351-UPD-0023 holds none, and the root's serve it. The channel kept there must show a version above the running one, and must not have expired (`expired`). If the slot is not as its manifest lists it, and the block store keeps the release staged there above the running version, that release is first written there again from the store (1.5, [below](#releases-kept-in-the-block-store-351-upd-0033)). Then it writes the record that boots that slot on trial with 1 to 9 tries, falling back to the running slot, and asks `init` to restart the machine. |
| `rollback` | The same check of the other slot. If that slot is not as its manifest lists it, and the block store keeps the last known good release, that release is first written there again from the store (1.4, [below](#releases-kept-in-the-block-store-351-upd-0033)). The channel kept there must show a version not below the highest minimum the updater has seen (the running slot's channel, and the last check). Then it writes the record that boots that slot on trial, falling back to the running one, and asks `init` to restart. |
| (the floor) | Not a request: once the running slot is confirmed, the updater raises the version floor to the highest minimum a checked channel named, if the running slot's own manifest names that version or above; never down. It writes the newer record again, one on, with that floor (`plan::raise`), and every other record it writes carries the floor on. Records of format 1 it names at start and does not read (`RECORD MIND/BOOT0 OF FORMAT 1, NOT READ`). The bootloader boots no slot below it ([slots.md](slots.md#the-version-floor-351-upd-0024)). A rollback to a release at or above the minimum stays allowed. |
| `status` | The running slot, whether it is on trial, its version, the slot the newer record names and whether it is confirmed, the version staged in the other slot, and the last failure. |
| `metadata` (1.1) | The channel the updater holds, checked with the release keys of its key file: the one it last read from the source, a bundle's, or else the one kept in the running slot. Its name, version, minimum and expiry, whether the expiry has passed, and whether it was read since the updater started. |
| `kept` (1.5) | The releases the block store keeps for the updater, newest first, at most 8; none without the store. For each: its version, the pin that keeps it, the tree's root, and the bytes of the files its manifest lists for a slot, each counted once (351-UPD-0035). Any client of the updater may ask. |
| `install-begin` (1.3) | From the Effector gateway's client (`BADGE_UPDATE_GATEWAY`) only, while `init`'s `remote-update` counts; any other client gets `denied` (751-UPD-0032). Begins a bundle of the size and SHA-256 named, in `MIND/<other slot>/INSTALL/BUNDLE.ZIP`, which neither the bootloader nor a fetch reads. The gateway's command ID goes into the log (MC-10.3). A bundle begun before is dropped, and so is one a restart left. |
| `install-write` (1.3) | The bundle's bytes at an offset no further than the bytes written so far: what lay at and past it goes, so that a cut transfer resumes. |
| `install-commit` (1.3) | Asks `remote-update` again, checks the bundle's size and SHA-256 (`digest`), reads it as a ZIP of stored entries (`malformed` otherwise: compressed, encrypted, ZIP64, a name that leaves the tree or repeats, a range outside the file), then stages the release it holds as `fetch` stages a source's, with the same checks, and removes the bundle. The bundle holds what `release.py bundle` writes: the channel `update.txt` names, its manifests and signatures, and the blobs they list. The gateway calls `apply` next, which asks `remote-update` again. |

The other slot is not written while the running slot is on trial and not yet confirmed on the disk: it is then the fallback. A request runs to its end before the next one is read.

**Confirming a trial.** On a trial boot the kernel restarts the machine at its deadline unless `init` confirmed the boot (351-KRN-0014).
- **It asks `init`.** Every second the updater asks `init` how the boot went (`init.wit` 1.4 `boot`, 000-KRN-0102; 351-UPD-0021). `init` answers once it has confirmed or refused the boot.
  - Confirmed: the updater writes the record that confirms the running slot at once, the same slot and fallback, confirmed (`[UPDATER] SLOT … CONFIRMED ON THE DISK: INIT CONFIRMED THE TRIAL BOOT`).
  - Refused: it writes nothing and does nothing automatic. The kernel restarts the machine into the other slot, which stays untouched.
- **When `init` cannot say** (an `init` before 1.4), the updater waits until 5 s past the kernel's deadline, on the same clock. If the machine is still running then, the boot was confirmed, and it writes the same record.
- Until the trial is settled it does nothing automatic.

**Records.** Each record goes into the file that does not hold the newer valid record, one past its sequence ([slots.md](slots.md)). It is written whole and in place, then read back. A sequence that could not count two further (a trial and its failure) is not written. `updater/src/plan.rs` chooses the record; its host tests run the bootloader's own encoding (`tests/updater_host.rs`).

## Releases kept in the block store (351-UPD-0033)

MC-9.3 asks that the recovery images and the objects they need be kept from ordinary cleanup. The updater keeps its releases in the block store ([docs/storage](../storage/README.md)) when the store's medium outlives a reset. At start it says which: `[UPDATER] STORE: RELEASES ARE KEPT IN THE BLOCK STORE`, or why not (no client, not mounted, a RAM disk).

- **A release is a tree** of the release layout (`updater/src/kept.rs`):
  - `keys/current`, the key file kept with it (351-UPD-0026);
  - `channels/<name>`, the channel kept with it;
  - `releases/<version>/<processor>/MANIFEST` and `MANIFEST.SIG`;
  - the blob of each file the manifest lists for a slot, under the first two digits of its name (`blobs/ab/ab…`), so that no directory is over a block.
- **Pinned by the updater's own owner.** Its client is badged `OWNER_UPDATER` ([351-STO-0006](../../issues-done/351-STO-0006-releases-pinned-in-the-store.done)). Collection never frees what a pin retains, and no client of another owner ends the pin, whatever its rights. No name points at a release: a name keeps its versions, which would hold a release past its pin.
- **When a release is kept:**
  - the running one, at start and once a trial is confirmed, if the store does not keep it yet. Its manifest must be the one booted and the boot key's, and its channel signed by the release keys of the key file the updater holds, naming it;
  - a staged one, once `fetch` or `install-commit` has written it and its files passed their checks.

  Each file is read from the slot (or, for an application a slot does not hold, from the root) and checked by size and SHA-256 as it is put. A blob a kept release already holds is taken from that tree, not put again. The log says what was put and taken: `[UPDATER] STORE: VERSION 6 KEPT AS PIN …, ROOT …: … FILES, … BYTES PUT FROM SLOT B, … TAKEN FROM A KEPT RELEASE, IN … MS`.
- **Which stay** (`plan::keeps`), after a confirmed boot and after a staging:
  - the running release;
  - the last known good: the newest below the running one and not below the minimum (the highest the updater has seen, or the records' floor), which a rollback may boot;
  - a staged release above the running one.

  The pins of the others end (`[UPDATER] STORE: VERSION 4 LET GO (PIN 1)`), and the log lists what stays (`[UPDATER] STORE KEEPS VERSIONS 6, 5`). Nothing ends while the running slot is on trial, since the other slot is then its fallback. While the running version is not known, nothing ends.
- **A damaged slot written again.** `rollback` checks the other slot first. If it is not as its manifest lists it, the last known good release the store keeps is written there again as from any source (`Source::Store`): its channel against the release keys of the key file the updater holds (351-UPD-0027) and its version, its manifest against the channel and the boot key, and every file by size and SHA-256. An application the running slot holds as listed is copied from there. The channel's expiry is not asked, as for a rollback. Then the slot is checked as the bootloader will check it, and booted on trial.
  - `apply` does the same for a staged slot that is not as its manifest lists it (351-UPD-0034), with the release staged there: only the version the slot's channel named at the updater's start or the last fetch, above the running one, and only if the store keeps it. Its channel's expiry is then asked as for any apply. Without such a release the apply is refused as before (`digest`, or `not-staged`).

## Recovery orders (351-UPD-0030)

The floor only goes up by the updater's own writes. The one way below it, short of writing the disk by hand, is a **recovery order**: `recovery/<channel>` on the source, one line of JSON (`channel`, `expires`, `floor`, `sequence`, `version`) and the signature lines of the key file's **recovery** keys, a role apart from the release and timestamp keys. At each check, right after the key file, the updater reads it, if there is one, and applies it when:
- the recovery keys of the key file it holds signed it to their threshold: `[UPDATER] RECOVERY ORDER 1 REFUSED: FEWER THAN 1 OF THE RECOVERY KEYS OF KEY FILE VERSION 1 SIGNED IT`;
- it is this channel's, has not expired, and its sequence is above the last one applied: an order served again is refused (`… REFUSED: NOT ABOVE ORDER 2, APPLIED BEFORE`);
- the running slot is confirmed, and the other slot holds the order's version, checked as the bootloader will check it, its channel signed by the release keys of the key file the updater holds (351-UPD-0027).

Then, in this order: the order goes into the account, `system/updater/recovery`, one line `<channel> <sequence> <version> <floor> <its SHA-256>` each, kept the way the key file is; the record of the other slot on trial with one try and the floor lowered to the order's (`plan::recover`) is written; and init restarts the machine: `[UPDATER] RECOVERY ORDER 2 OF CHANNEL stable APPLIED: VERSION 5 IN SLOT B ON TRIAL, THE FLOOR 6 LOWERED TO 5; …`. A refusal is said once and the check goes on. After a revocation, an order can point to a release the new release key signed. The channel still names what it names: until the publisher changes it, a device back on an older version holds that channel's offer.

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
- **Freezes.** A source that keeps serving the same unexpired channel holds a device on it until its timestamp expires: a day by default, renewed only with the timestamp key (351-UPD-0028), not for the channel's weeks.

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
- **Without the file** the updater has no source of its own, and `check` and `fetch` asked of it answer `no-source`. It still takes the cable's directory, below.
- **The cable's directory, `data/link/rel/`, is a source always** (801-UPD-0039, the maintainer's decision of 2026-11-14 for 801-LNK-0044: the cable updates all code with no permission word). `link` writes it as `rel/`, and `scripts/link_update.py` fills it.
  - **When:** every 20 s while no trial waits, it opens `timestamps/<channel>` there. Once that file changed since the last look, the release is staged and applied, whatever `update.txt`'s `source` and `automatic` say. A directory with no timestamp costs one open and logs nothing.
  - **The checks are any source's:** the key file as the next of the one trusted, the timestamp's and the channel's signatures and expiry, the manifest's boot-key signature, each file's size and SHA-256, the floor and the minimum (MC-9.2–9.4). So the cable installs only what the updater's keys signed. A release not newer than the running one is logged and left.
  - **The channel** is `update.txt`'s (`stable` without it). The timestamp's and the recovery orders' records are kept by channel, so the cable and the network source of one channel share them, and a newer release from either wins.
  - **The log** names it once at the start: `[UPDATER] SOURCE data/link/rel (THE CABLE'S, 801-LNK-0044): ALWAYS, AUTOMATIC APPLY`. An `update.txt` that names the directory as its `source` leaves it to this path.
- **The automatic run** starts once the trial (if any) is confirmed. When the source does not answer, it is tried again after 30 s.
- **The source** may be an HTTPS server or a directory with the layout of `scripts/release.py`. Both go through the same checks.
- **HTTPS** runs over the flow grant the network policy gives `updater`, so `netpolicy.txt` must name the server.

### A release's own period: period.txt (351-UPD-0036)

`every` is no anchor of trust, as `source` and `pin` are, so a release may carry it. Then an update, over the cable, through Effector or from the server, changes it, and the stick need not be written again.

- **Where:** `period.txt` in the slot (`MIND/<slot>/period.txt`), listed in the slot's signed manifest. It is not `update.txt`, which is the root's: a release's `update.txt` would be a file of the root (`plan::kind`).
- **What it holds:** one line `every SECONDS`, with `#` comments and blank lines around it (`plan::period`). Any other line, a second `every` or none refuses the file.
- **What the updater does at start,** for the slot it booted:
  - The slot's manifest must be the one booted, its digest the launch record's.
  - `period.txt` must match the manifest's line (size and SHA-256).
  - It must hold `every` alone.
  - Then its period replaces `update.txt`'s: `[UPDATER] EVERY 60 S, BY SLOT B'S period.txt (update.txt'S: 21600 S)`.
  - Otherwise `update.txt`'s stays, and the log says why: `… IS NOT AS THE MANIFEST LISTS IT`, `… REFUSED (source …)`, or a manifest that is not the one booted.
  - `source`, `pin`, `channel`, `automatic` and `tries` stay the root's alone.
- **Staged and applied** as the slot's other files:
  - `fetch` stages it with them, and removes a slot's old one that the new manifest does not list.
  - `apply` checks it as listed and holding `every` alone, and refuses a slot with one the manifest does not list.
  - A release whose period is refused is not booted: `[UPDATER] SLOT B: period.txt REFUSED (…)`.
  - The block store keeps it with the release's other slot files (351-UPD-0033).
- **The build writes it when asked:** `MIND_UPDATE_EVERY=60 ./02_build.sh` (and `scripts/build_aarch64.sh`) writes `period.txt` before the manifest is signed.
  - Without the variable, a stale one is removed.
  - `make_usb_image.py --slots` and `boot_slots.py layout` put it in slot A.
  - `release.py` publishes it as a blob, as it does every listed file.

## Tested

- **The `update` check** (part of the `updater` suite; x86, aarch64). The image holds release 5 in slot A, confirmed, with its channel kept there. `update.txt` names the test server (`httpd` with its test hooks, [server.md](server.md)) over HTTPS, trusted by its pinned key, with `automatic apply` every 20 s.
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
- **The `updatestore` suite** (351-UPD-0033; x86, aarch64; run with `updater`) uses a blank store disk, and `update.txt` asks nothing automatic.
  - At its start the updater keeps release 5 in the store.
  - `update fetch` stages release 6 and keeps it. It puts only the two files release 6 changes (`uptime.elf` and the loader) and takes the others from release 5's tree.
  - The store logs both pins with the updater's badge, 23. The shell's client holds no pins and is refused the updater's (`Rights`).
  - Applied and confirmed, the store keeps both releases. A collection frees an object nothing retains, and both trees read back whole with `blocks check <cid> tree`.
  - Before the apply, with the machine off, a byte of slot B's staged kernel is changed on the disk (351-UPD-0034). With no network, `update apply` writes release 6 back from the store, and slot B boots on trial and is confirmed. Its kernel is then as published.
  - With the machine off, slot A's kernel and loader are damaged on the disk. `update rollback` writes release 5 back from the store, and slot A boots on trial with release 5's manifest and is confirmed. Its files are then as published.
- **Host tests** (`tests/updater_host.rs`) cover each record the updater writes, followed through the bootloader's choice:
  - an update's trial;
  - its confirmation;
  - a failed trial, which leaves the fallback running;
  - a rollback;
  - no valid record;
  - a sequence at its end.

  They also cover how a manifest's files split into boot files, applications and the root's shared files, and when metadata counts as expired: from the second of its expiry on, or always on a clock that cannot be read. Since 351-UPD-0033 they cover the releases kept in the store too:
  - which stay;
  - a kept tree read back by the layout's paths;
  - two releases sharing their blobs, with 400 blobs under their prefixes;
  - trees out of shape or damaged, refused.

## Not provided yet

- **A caller of `idl/update.wit`.** No program holds a client of the updater: the shell's `update` command is the tools track's (351-APP-0029, numbered on its branch from [requests-APP.md](../../issues/requests-APP.md)), and the slot for its client is asked of the kernel track ([requests-KRN.md](../../issues/requests-KRN.md)). So `check`, `fetch`, `apply`, `rollback`, `status` and `metadata` run only through the automatic policy in the tests. `rollback`, `status` and `metadata` have not run on the platform, nor `apply`'s refusal of an expired staged release: the automatic run checks the source first.
- **The directory source's other refusals, in QEMU.** The `linkupdate` suite (351-KRN-0141) fetches a release from `data/link/rel/`, boots it on trial and confirms it, and refuses an expired channel and one with a changed signature. An older channel, a changed boot file and a resumed file go through the same `stage()` as over HTTPS, where the `update` suite shows each, but none of them has run from a directory.
- **The Effector gateway's own side of `install`** (751-EFF-0023). The `install` suite (run with `updater`, 751-UPD-0032) streams bundles through a stand-in: `tests/updater_stub` in effector_gw's place, holding the gateway's client.
  - With `remote-update` off, every `install-begin` and `install-commit` is `denied` and logged.
  - With it on:
    - a bundle with one byte of a boot file changed is refused (`digest`);
    - so is one whose manifest is signed by another key (`signature`), and one cut after its first piece (`digest`);
    - release 6's bundle, a piece written twice, is staged in slot B and removed;
    - `apply` boots slot B on trial, and the trial is confirmed.

  It has passed in QEMU on x86 and aarch64. The bundle's reader also runs in the host tests (`tests/updater_host.rs`) on a bundle `release.py bundle` wrote and on hostile archives. No client of another badge has run against the install calls: the shell's `update` command is not there yet.
- **Kept releases, what is not there yet** (351-UPD-0033):
  - The shell's `update` does not show which releases the store keeps: `kept` (1.5) gives them to a client, and the shell's command is the tools track's to add ([requests-APP.md](../../issues/requests-APP.md)). The log shows them.
  - A staged release the store does not keep, or whose slot's channel cannot be read at the updater's start, is not written back by `apply`: `fetch` writes it again from the source.
  - A collection in the minute between a block's put and the tree's pin may free the block. The pin is then refused (`incomplete`), and the next start keeps the release.
  - The updater's account counts each release whole, so three releases need three quarters of a medium four times a release. A quarter is kept for it whatever other owners retain (351-STO-0043); past that it uses the room the others leave.
  - None of it has run on real hardware.
- **SSH** (351-NET-0004).
- **Rollback protection** beyond the version check: a fallback is booted whatever its version (351-UPD-0009, 0011).
- **Key rotation and revocation** (351-UPD-0009). The bootloader is not updated (351-UPD-0010).
- **Slot images by default.** `make_usb_image.py --slots` makes an image the updater can update ([write-disk.md](../write-disk.md), 351-UPD-0016); without the option an image boots from its root and has no slot to fill.
- **The clock.** Expiry is only as good as the system clock, and the updater has no second source of time.
