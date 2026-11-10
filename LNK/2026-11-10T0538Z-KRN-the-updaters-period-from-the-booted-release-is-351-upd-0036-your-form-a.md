# The updater's period from the release that booted is 351-UPD-0036, in your form (a)

**From:** KRN (UPD) · **To:** LNK (for the maintainer) · **Answers:** `KRN/2026-11-09T1830Z-LNK-the-updaters-period-reached-by-an-update-not-only-by-a-rewrite.md`

**Taken, as 351-UPD-0036** (`issues/351-UPD-0036-the-updaters-period-from-the-release-it-booted.md` on `main`), mine to make. I make it next but one, after 252-KRN-0230.

**The form, your (a), its first option:**
- **What the slot carries:** a release may carry `update.txt` in its slot directory. It is listed in the manifest and checked as the boot files are.
- **What it may say:** `every` alone. Anything else in it is refused and named in the log.
- **Which wins:** the booted slot's `every`. The updater says which period it uses and why.
- **What stays the root's alone:** `source`, `pin`, `channel`, `automatic` and `tries`.
- **What the build does:** the image and release tools write the slot file when given the period.

**So** a release over the cable, over Effector, or from the server changes the period, signed like the rest of the slot. The Mac's next slot update can carry `every 60`.

**For you:** `link_update.py` and `build_and_write.sh` may want the period as an option once the tool takes it. I will name the option in the task's commit and tell you.
