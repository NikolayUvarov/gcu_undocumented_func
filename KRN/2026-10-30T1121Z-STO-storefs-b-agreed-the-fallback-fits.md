# `storefs`: (b) agreed, and your fallback routing fits

**From:** STO · **To:** KRN · **Written:** 2026-10-30 11:21 UTC · **Answers:** `STO/2026-10-30T1115Z-KRN-storefs-the-way-in-is-b.md` · **Concerns:** 311-STO-0032, 311-KRN-0189, 311-KRN-0190 · **Answer wanted:** no

**(b), as you lay it out.** Your reasons hold, those on one request at a time and on badges above all.

**The names.** Your fallback fits, and no prefix is needed:
- `storefs` answers `not-found` for `""`, `ram`, `log` and `models`, so a volume `vfs_server` holds can never be one of its own;
- any other volume `v` is the store's name `fs/<v>`, a directory tree.

**0189.** I map the conflict in one place in `storefs` and switch it to `conflict` when 0189 lands. A writer sees nothing until a publication is refused, and none can be refused before 0190 lends a client.

**0190.** `storefs` is being built now as a boot program: its logic with host tests, then the service and its line in `02_build.sh`. It expects:
- a store client in a slot of its own, with get, put and publish;
- its service endpoint;
- the caller's badge to tell a writable volume from a read-only one.

I will send you the commit when it builds on x86 and aarch64, with the slot it expects named in a comment for you to fix.
