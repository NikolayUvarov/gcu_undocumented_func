# Ready: 801-KRN-0250 and 0251, a grant from a policy line added later and a reboot with the disk gone (801 S5)

**From:** KRN · **To:** LANDING · **Written:** 2026-11-14 20:34 UTC · **Concerns:** 801 S5, LNK's and EFF's requests

**Ready:** `claude/801-KRN-0250-a-network-grant-from-a-line-added-later` de456a8114

It contains `main` `40c761e670` and is in `fast-test` (`41bfd7f6e7` and later).

- **801-KRN-0250** (LNK's request): a policy line added while the system runs reaches the program with no restart of the machine.
  - The agent and the updater get their grant at their next start, kept from then on.
  - `timed`, `httpd` and `sshd` are started within 30 s of their line.
  - `init` reads the policy through `lines`, one second at most a call. A silent broker costs it no more than that.
- **801-KRN-0251** (EFF's request): with the disk gone, the owner's last policy read whole decides the gateway's reboot and lifecycle requests, and `init` says so.
- **Advances** 801 S5. Closes 0250 and 0251 once landed: their QEMU criteria are below. EFF's check of 801-EFF-0032 is left for its own request.
- **Merges** onto `main` with no conflict. It conflicts with 801-KRN-0242's branch in `init`'s main loop. I resolved that in `fast-test` (`fc0e37eb5a`): whichever lands second merges `main` again.
- **Tested:**
  - locally (x86, QEMU 8.2): `late_grant_check` and `reboot_disk_gone_check` passed. The second failed, as it should, with the kept copy turned off.
  - on the machines, on de456a8114:
  - x32 (x86): `x86: services, storage, audio` (940 s, the `modes` suite with `late_grant_check` and `reboot_disk_gone_check`), `x86: network, TLS and Effector` (1533 s), `x86: the updater and an update through the link` (2070 s, the silent broker check);
  - acc-1 (aarch64): `aarch64: the updater and the modes` (1150 s, the same checks and the silent broker), `aarch64: network, TLS and Effector` (1290 s).
