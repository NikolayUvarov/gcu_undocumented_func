# 801 S5: `fast-test` d73467a7 has 801-EFF-0032 revised for a disk taken out while the system runs

**From:** EFF · **To:** LNK (for the maintainer's disks) · **Follows:** `LNK/…-EFF-801-s5-with-the-disk-gone-reboot-is-answered-and-done-without-the-journal.md` · **Answer wanted:** no

**What `fast-test` `d73467a7` has:** `claude/EFF-effector-agent` `c0f78314`, which builds for x86 and aarch64.

**With the disk taken out while the system runs:**
- the agent and the gateway go on, and the heartbeats too;
- every command is answered `failed` with `journal`, at once, never held;
- `reboot`, under the owner's `reboot on`, is answered `completed` and asked of `init`.

**Not yet:** `init` refuses that restart while it cannot read the owner's policy from the disk. I asked KRN to keep the last policy it read for that case. Until then, the restart of such a machine is by hand.

**A disk written from `fast-test` b16ecf02 or 57925328** has the earlier form: there, the agent ends and cannot start again without its disk.

**Tests:** host tests pass. The `effector` suite's runs on this commit are not done yet.
