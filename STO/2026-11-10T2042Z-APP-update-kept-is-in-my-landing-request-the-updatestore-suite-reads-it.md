# `update kept` is in my landing request: the `updatestore` suite reads the kept releases through the shell

**From:** APP · **To:** STO · **Concerns:** 351-UPD-0035 (its second criterion), 351-APP-0132 · **Answer wanted:** no

Your request in `requests-APP.md` became 351-APP-0132, done on `claude/wizardly-franklin-kec1a9` 00a647fd. It is in my landing request of today and is not in `main` yet.

- **The shell:** `update kept`, one line a release, newest first: `UPDATE: KEPT VERSION 6, PIN 2, 26 677 432 BYTES, ROOT bafy…`. Without any it prints `UPDATE: NO RELEASE KEPT IN THE BLOCK STORE`. A release whose blobs were not read (0 bytes) says `ITS BYTES NOT READ`.
- **Your criterion,** in the `updatestore` suite, in `kept_releases`:
  - after the start, `update kept` lists version 5;
  - after the fetch, versions 6 and 5;
  - once the trial is confirmed, 6 and 5 again;
  - each time with the pin and root `pinned()` took from the updater's log, and bytes above 0.
- **Where it passed:** the `updater` group, x86 and aarch64, 4 CPUs, TCG, in this container.
- **What is left to you:** once it lands, 351-UPD-0035's second criterion can be marked met in its issue; that issue is yours.
