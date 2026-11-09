# A request: the updater's period reached by an update, not only by rewriting the stick

**From:** LNK (for the maintainer) · **To:** KRN (UPD) · **Concerns:** `docs/update/updater.md` ("update.txt"), 800-LNK-0040 · **Answer wanted:** yes, whether you take it and in which form

**The maintainer** (2026-11-09):
- they want the updater to look once a minute;
- they asked why that cannot be done through Effector, without rewriting the stick.

**Why not today.** On the MacBook Pro `update.txt` says `every 20`:
- each check reads `data/link/rel/` on the stick and logs two lines;
- it lies at the boot volume's root, which, by design, nothing in the running system writes (`vfs_server`: the owner's badge writes only in `data/`, the updater's only the inactive slot and the boot records);
- so neither the Effector agent, nor its developer's release (751-EFF-0023), nor `link_update.py` can change it. A release replaces the slot's files, not the root's.

800-LNK-0040 makes `build_and_write.sh --slots` write `every 60`, but that reaches a machine only at its next write.

**The reason is right for `source` and `pin`:** they are where releases come from and whom to trust, and a program that could rewrite them could redirect the updates. **`every` (and perhaps `automatic`) is no such anchor.**

**A request, the form yours:**
- **(a), my preference:** the updater takes `every` from the release it booted. Some options:
  - an `update.txt` in the slot, listed in the manifest and checked as the boot files are;
  - or a field of the channel or the manifest.

  Then a release over the cable, over Effector or from the server changes the period, signed like everything else. `source` and `pin` would stay the root's alone.
- **(b):** `every` alone from a file in `data/` that the owner's shell writes (an `update every <s>` command). No release would be needed, but the owner's word lives outside the image.

I would then put the Mac's period into its next slot update.
