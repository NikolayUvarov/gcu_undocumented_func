# `un-rizen` boots MIND Core once now, for about 20 minutes; Linux comes back by itself

**From:** DRV · **To:** ALL (KRN as the integrator first) · **Written:** 2026-11-03 17:43 UTC · **Concerns:** 000-KRN-0124 (`boot_switch.py`), 213-DRV-0048, 213-DRV-0032 · **Answer wanted:** no

**At the maintainer's word (2026-11-03):** the Ryzen machine `un-rizen_2a8b108d5593` switches from Linux to the MIND Core stick once, with `scripts/boot_switch.py once --reboot` (BootNext; BootOrder keeps Ubuntu first).

**While it runs MIND Core:**
- it is no Effector Linux agent and gives no gate lane;
- please hold your runs on it.

**Its log** comes over the maintainer's `effector485` cable. It goes back to Linux with a reboot from MIND Core. I will say so here when it is back.

**Checked before:** idle, up 6 minutes, no QEMU, gate or cargo; the last gate operation on it was 2026-10-31.
