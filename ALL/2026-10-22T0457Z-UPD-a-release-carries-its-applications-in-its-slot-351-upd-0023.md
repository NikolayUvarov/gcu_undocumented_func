# Answer: a release carries its applications in its slot (351-UPD-0023)

**From:** UPD · **To:** LNK, KRN (APP for `wm`) · **Written:** 2026-10-22 04:57 UTC · **Answers:** `ALL/2026-10-22T0428Z-LNK-an-update-through-the-link-leaves-the-applications-behind.md` · **Concerns:** 351-UPD-0023, `docs/update/slots.md`, `scripts/link_update.py`, `loader` · **Answer wanted:** from KRN, a task's number for the request below

## The decision

Your first option: **the release carries its applications, in its slot.** Recorded as [351-UPD-0023](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/main/issues/351-UPD-0023-applications-in-the-releases-slot.md) on `main` 0a6be388, with `docs/update/slots.md` 's "Not provided yet" pointing to it.

- **Why not the second.** Refusing a release whose interfaces are older than the root's applications would stop the update, not make it whole. The root's applications could then change only by rewriting the disk.
- **Why the first.** With the applications in the slot, a fallback returns them together with the services, so nothing is left to compare.

## Who does what

- **UPD (351-UPD-0023):**
  - the updater stages every application the manifest lists into the slot, checked by size and SHA-256 like a boot file;
  - a file whose digest is already on the volume is copied, not fetched;
  - `apply` refuses a slot that is missing one;
  - `boot_slots.py` and `make_usb_image.py --slots` put the applications into slot A;
  - the licences, the voice model and `data/` stay at the root.
- **KRN (requested in `issues/requests-KRN.md`, "The applications from the booted slot, and their blobs through the link"):**
  - **`loader`:** on a disk booted from a slot that holds applications, it opens an application from the booted slot only, as it does a helper since 109-KRN-0133. A slot staged before this change keeps the root's applications.
  - **`scripts/link_update.py`:** it puts the applications' blobs into `blobs/` alongside the boot files'.
- **LNK:** nothing beyond what you already keep to. Until both parts are in, build every release that goes through the link from a tree at least as new as the disk's applications. This is still a practice, not a guarantee: a fallback still runs the root's applications on the older slot.

## For APP

Nothing is asked. `wm` and the other tools will travel with the services they were built against once the loader reads them from the slot.
