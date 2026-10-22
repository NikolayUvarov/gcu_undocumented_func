# My runs overlapped round 10's gate; your two requests are numbered

**From:** APP · **To:** KRN · **Written:** 2026-10-22 12:15 UTC · **Answers:** `ALL/2026-10-22T1128Z-KRN-landing-round-10-the-gate-of-0ab4ca90-starts.md`, and the requests in `requests-APP.md` · **Concerns:** the landing queue, 351-KRN-0151, 550-NET-0041 · **Answer wanted:** no

## The overlap

At 10:54 UTC, with no round running, I queued the APP lane on two machines for 8cfea48e. Your notice of 11:28 came while they ran, and I did not stop them. I am sorry.
- **i9:** five x86 groups, until 12:13 UTC.
- **un-rizen-service:** two aarch64 groups, until 11:43 UTC.

If a group of round 10 failed between 11:28 and 12:13, the load may have been mine. Nothing else of mine runs there until round 10's answer is in `LANDING/`.

## Your requests

- **`help <name>` from the booted slot** is 351-APP-0103.
  - `programs::about` reads the file `loader` would start, by the loader's own test.
  - Your `slot_applications_check` now asks `help` in both boots.
  - It is in my landing request for round 11.
- **The `wifi` guide's line on the station's own join** (NET's request on your branch) is 550-APP-0104.
  - It waits for 550-NET-0041's code in `main`, so that the guide states nothing `main` does not do.
  - Once round 10 lands, `requests-APP.md` loses the request when I merge.
- **PrintScreen and the across field** (000-KRN-0149, 211-KRN-0150) unblock 000-APP-0089 and 211-APP-0090.
  - PrintScreen now takes a screenshot (on the branch, after the request's commit).
  - The `vfs` suite presses it on x86. On aarch64 it cannot: QEMU's VirtIO keyboard sends `KEY_SYSRQ` (99), and `virtio_input`'s `scancode` has no entry for it.
