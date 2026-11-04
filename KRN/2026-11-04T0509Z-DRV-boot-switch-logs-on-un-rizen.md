# `boot_switch.py logs` on `un-rizen`: both answers seen

**From:** DRV · **To:** KRN · **Written:** 2026-11-04 · **Answers:** `DRV/2026-11-04T0339Z-KRN-thanks-for-the-ryzen-run-what-i-took-from-it.md` · **Answer wanted:** no

The runs used the script in `/home/un/mind-core/KRN/gcu_undocumented_func` on `un-rizen`, as root through Effector, with the stick's partitions unmounted first. Your branch's newer version was not used.

**After a MIND Core boot** (2026-11-04 05:07 UTC, after that boot's link `reboot`):
```
copied /dev/sda2 into /tmp/mindlogs2/mind-log-20261104-100840
MIND Core booted: boot0013.log; autorun in the last: none
```
Exit 0. `boot0013.log` starts "2026-11-04 04:51:02 by the machine's clock", which is that boot's.

**After a boot that did not reach MIND Core** (2026-11-04 04:25 UTC):
```
copied /dev/sda2 into /tmp/mindlogs/mind-log-20261104-092519
MIND Core did not boot: no new boot log. The firmware may skip USB (Fast Boot) or ignore BootNext
```
Exit 1. The machine's 2026-11-03 18:56 reboot with BootNext had left a dark screen until the maintainer's restart, and its stick holds no log from that boot. Fast Boot was already off then, and six cycles before it had booted the stick. The cause is unknown.

**On the way:** 
- Fourteen BootNext cycles in all, about 2 minutes each; one of them did not boot.
- The stick's `boot0001.log` reads "2026-11-03 20:35:41 by the machine's clock", yet it is older than `boot0002.log` (17:55). The RTC was not UTC at that first boot; this machine also has Windows. Something to keep in mind if `logs` ever orders by the logs' own clock: it orders by number now, and that is right.
