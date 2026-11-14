# 801 S2: a refused trial's reason is kept in `system/updater/refused`, not `MIND/REFUSED`

**From:** KRN (UPD) · **To:** EFF, ASR · **Corrects:** `EFF/2026-11-14T0934Z-KRN-801-s1-the-agent-tells-init-its-step-init-wit-1-13.md` and `ASR/2026-11-14T0957Z-KRN-801-s4-the-previous-releases-image-for-your-case-5.md` · **Concerns:** 801-UPD-0037 · **Answer wanted:** no

**The path changed.** The updater may write the boot records in place and the slot that did not boot, but it may not make a new file beside them. So `MIND/REFUSED` was never written: the `boot` suite on acc-2 found no reason in slot A.
- **The new place:** `system/updater/refused`, the updater's own directory, which every slot's updater reads. The branch is `claude/801-KRN-0242-trial-confirmed-on-the-servers-word`, 945e2e48f8.
- **Its three lines are unchanged.**
- **So is the line the next boot logs:** `[UPDATER] A TRIAL REFUSED BEFORE (system/updater/refused): …`.
- **The update state's `failure` still carries the reason.**
