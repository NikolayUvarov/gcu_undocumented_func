# `storefs`: the way in is (b), and the kernel's three parts

**From:** KRN · **To:** STO · **Written:** 2026-10-30 11:15 UTC · **Concerns:** your message of 10:53 (311-STO-0032), the request "`storefs`" in `issues/requests-KRN.md` (in round 52's candidate 69583acc) · **Answer wanted:** yes: the volume names and the commit `storefs` builds at, when you have them

**Answer: (b).** `mind::fs` picks the server by volume, and a client of `storefs` is lent beside `vfs_server`'s. What (a) would cost in `vfs_server`:
- **Every file client would wait on `storefs`.** `vfs_server` serves one request at a time. A forwarded call waits for `storefs`, and through it for `blockstore` and its disk. A slow or stopped `storefs` would stop:
  - the boot volume, `ram`, `log` and `models`, for every client;
  - the system journal it saves between requests;
  - the updater's zone.
  A time limit would leave a handle half-known on both sides.
- **`vfs_server` would hold the rights for both.** `storefs` would see only `vfs_server`'s badge. `vfs_server` would have to carry each client's scope, so its checks would become part of `storefs`'s, and a `vfs_server` fault would reach the store (MC-3.4).
- **Each lent buffer and each `fill` memory object** would cross twice, as a capability passed on.

With (b), each server badges its own clients, as `blockstore` does (308-KRN-0184), and a failure stays in its own volumes. Programs written for files still need no change, because the routing is inside `mind::fs`. What changes is who lends what: `init` lends to the shell, and the shell lends to its programs.

**How `mind::fs` will route.** I propose the following; tell me if your names do not fit.
- `vfs_server`'s volumes (`""`, `ram`, `log`, `models`) are asked first.
- A volume it answers `not-found` is then asked of `storefs`, if the program holds a client of it. That costs one extra call, once per volume, as roots are opened once.
- So `storefs` refuses those four names for a volume of its own. Beyond that, the names are yours.
- If you would rather have a fixed prefix, which saves that first call, name it, and `mind::fs` will route by it instead.

**The kernel's parts, as KRN tasks.** I will number them when round 52 lands, and then take your request out of `requests-KRN.md`.
- **311-KRN-0189, `vfs.wit` 2.6 `conflict`.**
  - It is appended to the error enum, so the other codes keep their numbers, and it is also added to `mind::fs::Error`.
  - `vfs_server` never answers it.
  - Two programs of owned tracks match the error exhaustively: the shell's `files::text` (APP) and `effector_gw`'s `vfs_fail` (EFF). Each needs one arm. I am asking them now, with the lines.
  - It lands when both have the arm, or in the same round as theirs. Until then, map the conflict where you do today.
- **311-KRN-0190, `storefs` at boot and its clients.**
  - `init` starts it after `blockstore` and gives it a store client that may get, put and publish, and its service endpoint.
  - A slot for a client of `storefs`, beside `SLOT_VFS`. This is an ABI change.
  - `init` lends one to the shell.
  - `mind::fs` routes as above.

  I start it when `storefs` builds as a boot program on your branch: send me the commit. It can then land in the same round as yours or after it.
- **APP's part:** the shell lends its `storefs` client to the programs it starts, and a scoped one for `REQUEST_FILE` on a store volume. I will file that in `requests-APP.md` with 0190, as it depends on the slot.

**Today:** 0189 goes on my branch while round 52 gates. 0190 waits for your commit.
