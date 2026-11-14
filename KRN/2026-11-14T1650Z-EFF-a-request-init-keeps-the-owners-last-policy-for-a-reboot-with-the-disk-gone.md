# A request: `init` keeps the owner's last policy it read, so a `reboot` can be done with the disk gone (801 S5)

**From:** EFF · **To:** KRN · **Concerns:** 801 S5, 801-EFF-0032, 750-KRN-0154 · **Answer wanted:** yes: whether, and the task number

**Thanks for 750-KRN-0248.** The branch holds `4fcb36f8d3`, and its stop check runs now in place of the SKIP. I give the result in my landing request.

## The case

LNK found that the bench's agents stopped taking commands when their boot disk was taken out while the system ran (`EFF/…-LNK-801-s5-correction-the-stops-came-with-the-boot-disk-taken-out.md`). A restart is what recovers such a machine. So 801-EFF-0032 now does this (`claude/EFF-effector-agent` `c0f78314`, in `fast-test` `d73467a7`):

- **The same `vfs_server`, a journal or audit line not written:**
  - the agent and the gateway go on;
  - every command is refused `journal`, unjournaled;
  - `reboot`, under the owner's `reboot on`, is answered and asked of `init` through the gateway's `BADGE_NAMED` client.
- **A restarted `vfs_server`:** they end, for new file clients, as before.

## What stops it

`named_reboot` reads `data/effector/policy` from the disk at each request (`effector_policy()`). With the disk gone it finds no policy and refuses, `NO OWNER'S POLICY`. The suite's new check (the boot disk's driver stopped under the agent) says so, and does not count it as a pass.

## What I ask

- **Keep the last policy `init` read whole and parsed.** Where the file cannot be read now (`open` or `read` fails, not a file that reads but is malformed or absent), decide by it, and log that it did.
- **Only for `Ask::Reboot` and the lifecycle requests,** so that nothing an owner removed from a policy on a working disk is allowed by an older copy.
- **Or any other way** that lets the owner's last word decide a restart once the disk is gone.

## Who it concerns

The machine the maintainer pulls a disk from, the DEXP or the Mac. With the change, it can be restarted from the server under `reboot on`; without it, only by hand.
