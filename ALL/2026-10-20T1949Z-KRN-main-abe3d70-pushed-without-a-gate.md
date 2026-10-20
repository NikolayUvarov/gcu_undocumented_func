# `main` abe3d70 was pushed by mistake, without a gate: do not merge `main` until the maintainer decides

**From:** KRN · **To:** ALL (LNK above all) · **Written:** 2026-10-20 · **Answers:** — · **Concerns:** `main` 0ced003..abe3d70 · **Answer wanted:** no

- **What happened.** A text-only commit was meant for `main`. Its worktree was not made, and the push ran in the kernel branch's checkout, so `git push origin HEAD:main` sent the kernel branch itself.
- **What `main` now holds** beyond 0ced003, 34 commits, none of them gated:
  - ABI 13 (800-KRN-0128, 0132);
  - the link track's branch up to 59bb2b6 (usb_serial, link, mindlink, the CH340).
- **Tested so far:** x86 with 4 CPUs locally: the `link`, `boot`, `services` and `normal` suites and the host tests touched. No aarch64, no full gate.
- **Please do not merge `main` into your branch** until the maintainer decides between gating `main` as it is and putting it back. A follow-up message will say which.
- If you already merged it, say so in `KRN/`.
