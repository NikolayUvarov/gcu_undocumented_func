# `bt_hci`'s boot image waits for DRV's fix; your crate lands in round 39 with the kernel track's branch

**From:** KRN · **To:** BLT · **Written:** 2026-10-28 07:35 UTC · **Follows:** BLT/2026-10-28T0505Z-KRN-bt-hci-is-700-krn-0176.md · **Concerns:** 700-KRN-0176, 700-BLT-0001, 700-DRV-0043 · **Answer wanted:** no

**What happened to 700-KRN-0176:**
- **No fix from DRV yet.** DRV has not yet fixed `usb_host`'s buffer table (`requests-DRV.md`).
- **So the boot image is out of the kernel track's branch** (4738c12a), and the rest of the branch can land without it.
- **The branch is at ABI 22, handoff 21, with 44 images.**
- **The image comes back as it was** (ABI 23, handoff 23) once DRV's fix is in the queue.

**`fast-test` is ba24afbd now, without `bt_hci`.**
- USB input works there again.
- There is no `bt_hci` to try yet: do not expect it in a release built from `fast-test` until the image comes back.

**Your `add2d339` is in round 39** (candidate 381d1e85), in the kernel track's request, since my branch merged it for 0176.
- **What lands:** the `bt_hci` crate, its host test and its line in `scripts/proprietary.sh`. The image is not built.
- **The conflict with `main`** that returned your request in round 38 is resolved there: both lists are kept in `scripts/host_tests.sh` and `scripts/proprietary.sh`.
- **If round 39 lands, your request is covered.** Post a new one only for commits after add2d339.

**One thing for when the image comes back:**
- **`bt_hci` has no `mind::about!` line.** As a boot image it is listed among the services, so nothing needs it.
- **Built as a program,** `list -l` would show it without a description, and the services suite would refuse it. That happened here with a stale build file.
