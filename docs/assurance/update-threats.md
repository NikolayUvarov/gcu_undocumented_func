# The update's threats

**Version:** 1.0 (2026-10-09, [351-ASR-0006](../../issues/351-ASR-0006-update-threat-model.md)) · **Constitution:** [v1.6](../../constitution/EN/MIND_CORE_Constitution_v1.6.md) MC-9.1–9.7, MC-11.10, MC-12.2, MC-12.3

What an attacker can do to MIND Core's updates, what stops each attack today (the task and the test), and what nothing stops yet.

**What exists:**
- releases are built, signed and published on a host ([docs/update/publishing.md](../update/publishing.md));
- the bootloader checks a signed boot volume and boots slots A and B with a trial and a fallback ([docs/update/slots.md](../update/slots.md));
- the firmware checks the bootloader under Secure Boot with our own keys ([docs/update/secure-boot.md](../update/secure-boot.md)).

**What does not exist yet:** the updater on the device ([351-UPD-0007](../../issues/351-UPD-0007-updater-service.md)). Every check a device would make on a channel is therefore absent, and the table says so.

**The keys.** A release signed with a test key, whose seed is public text, is checked for accidents only, never against an attacker (publishing.md, "Keys"). The table assumes real keys.

## Attacks and what stops them

| Attack | Stopped by | Test | Not covered |
|---|---|---|---|
| **Physical rollback:** someone with the disk puts an older, validly signed release on it | Nothing at boot. The boot manifest carries no version. Secure Boot's `dbx` revokes a bootloader known to be bad (351-UPD-0012, 351-KRN-0028) | `tests/dbx_update_smoke.py` (a revoked bootloader is refused) | **Not covered:** a lower version with an unrevoked bootloader boots. Needs a version floor the disk cannot hold ([351-UPD-0011](../../issues/351-UPD-0011-version-floor-in-the-tpm.md), TPM NV counter) and the rollback policy ([351-UPD-0009](../../issues/351-UPD-0009-rollback-policy-and-key-roles.md)). The MacBook Pro has no TPM at all |
| **Rollback over the network:** a server or a man in the middle offers an older release | The channel names a version and a minimum; `publish` refuses a version not above the channel's | `release_test.py`: `test_a_version_not_above_the_channel_is_refused` (publisher side only) | **Not covered on a device:** nothing on the device reads a channel yet (351-UPD-0007, 0009) |
| **Freeze:** old metadata served forever, so no update arrives | The channel carries an expiry, which the host checker enforces | `release_test.py`: `test_changes_are_refused` (an expired channel) | **Not covered on a device** (351-UPD-0009) |
| **Mix and match:** files of different releases on one volume | One manifest lists every boot file with its size and SHA-256; the bootloader checks each before use. On the server, the channel names each manifest's hash and blobs are stored by hash | QEMU `boot` suite: "signed boot volume: a changed kernel or service, a changed manifest, another key's signature and no signature each stop the bootloader"; `release_test.py`: `test_changes_are_refused` | Programs that `loader` reads later from the boot volume are not checked against the manifest |
| **Arbitrary software:** a volume or bootloader from someone else | Ed25519 over the boot manifest with the built-in key (350-UPD-0003); Secure Boot with our keys (351-UPD-0012) | QEMU `boot` suite (as above); `tests/secure_boot_smoke.py` (a bootloader signed with another key is refused) | Secure Boot has run in OVMF only, not yet on the maintainer's PC (351-UPD-0012) |
| **Endless data:** a download that never ends or exceeds what the metadata says | The bootloader reads a file into a 4 MiB buffer and refuses a larger one. The manifest states every file's size | QEMU `boot` suite (a corrupt or missing boot file is reported) | **Not covered on a device:** downloads stopped at the stated size and per-field limits on channel data (351-UPD-0007) |
| **Slow retrieval:** a server that answers too slowly to finish | `download` gives up on a connection that sends nothing for its idle limit (`IDLE_MS`) and resumes later (351-NET-0001) | — | **Not covered:** an overall deadline, so a server that trickles a byte at a time is never cut off (351-UPD-0007) |
| **Key compromise** | The boot key, the release key and the SSH login are separate, so stealing one does not give the others (MC-9.6) | `release_test.py`: `test_the_release_key_is_not_the_boot_key` | **Not covered:** rotation, revocation and the compromise protocol (351-UPD-0009) |
| **Boot-record tampering:** someone with the disk edits the records to pick a slot, confirm a trial or keep one going | The records hold only a CRC against torn writes; they are not authenticated | `tests/boot_slots_host.rs`; QEMU `boot` suite, slot tests | **Not covered:** an attacker with disk access chooses the slot, within what verifies. Like physical rollback, this needs a measured or sealed state (TPM) |
| **A failed update leaves no system** | Two slots, a trial with a try count written to the disk before the slot runs, a fallback, confirmation by `init` (351-UPD-0006, 351-KRN-0014) | QEMU `boot` suite: a trial confirmed, an unconfirmed trial falling back, a damaged slot not loaded, a torn record ignored | Power lost at every step of an update: [351-ASR-0005](../../issues/351-ASR-0005-power-loss-during-update.md), waiting for the updater. The count-down stops at the largest sequence number (finding 2 below) |
| **Malformed metadata** making a parser fail before or after its signature check | The bootloader parses the manifest only after its signature; the boot records only after their CRC | Fuzzing, next section | The device's channel parser does not exist yet. The host checker raises on malformed signed channels (finding 1) |

## Fuzzing of the parsers

The fuzzers have fixed seeds and counts, and `MIND_FUZZ_SEED` and `MIND_FUZZ_ITERATIONS` widen a search. A run is evidence of the inputs it made, not a proof (MC-12.2).

- **The bootloader's manifest reader**, `bootloader/src/verify.rs`: [`tests/manifest_fuzz/`](../../tests/manifest_fuzz/), `cargo test --release`, with the bootloader's own `ed25519-dalek` and `sha2`.
  - 20 000 mutated manifests, each signed with a fixed test key.
  - Checked: the reader never panics; it refuses a wrong signature and another format; a file it confirms has a matching `file` line.
  - **No finding.** A reader changed to skip the hash was caught.
- **The boot records,** `bootloader/src/slots.rs`: [`tests/update_fuzz_host.rs`](../../tests/update_fuzz_host.rs), 200 000 records and 200 000 pairs.
  - Checked: the reader never panics and accepts only its one encoding; the choice never panics, always boots a slot, and writes records that count as newer.
  - **Finding 2** below.
- **The host's channel checker,** `scripts/release.py` `check`: [`tests/update_fuzz_test.py`](../../tests/update_fuzz_test.py), 3 000 channel files signed with the test release key.
  - Checked: `check` answers None or a reason and never raises.
  - **Finding 1** below.

### Findings (2026-10-09), sent to the update track

1. **`release.check` raises instead of answering** for a validly signed channel whose body is not what `publish` writes. Seven kinds of exception appeared:
   - `UnicodeDecodeError` and `JSONDecodeError` from a body that is not UTF-8 or not JSON;
   - `KeyError` from a missing field;
   - `TypeError` and `AttributeError` from a field of another type;
   - `ValueError` from an `expires` in another format;
   - `ReleaseError` from a minimum above the version.

   The channel is not accepted, so nothing is bypassed. But the checker's promise is broken, and the device's updater will parse the same format.
2. **The trial's count-down stops at the largest sequence number.** `plan` and `spent` write `sequence + 1`.
   - At `u64::MAX` this panics in a debug build. The bootloader's release build does not check for overflow, so it wraps to 0.
   - The new record then counts as older than the one followed, and an unconfirmed slot keeps booting on trial without its tries going down, never falling back.
   - Reaching that number needs a crafted record; whoever can write one can also mark a slot confirmed. The defect is in the count-down's logic, not a new way in.

## Not covered by this document

The network path's TLS and SSH (`NET`), the updater's storage and its update zone (351-UPD-0008), and the bootloader's own update (351-UPD-0010).
