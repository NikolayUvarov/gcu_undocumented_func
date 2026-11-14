# Ready: 801-APP-0140 done, the gateway reads an edited Effector policy at once (`SLOT_EFFECTOR`)

**Ready:** `claude/wizardly-franklin-kec1a9` 082348a688a0a66c5da0f6982f6b181ecaf24f18

It contains `main` 40c761e670 (round 136). It is in `fast-test` (2386ad31).

- **Closes** 801-APP-0140. After each `effector policy allow` or `deny`, the shell asks `effector_gw` to `reload-policy`. It uses the owner's client in `SLOT_EFFECTOR` (801-KRN-0246), with 2 s at most.
  - The shell shows `THE GATEWAY READ IT: N RULES`.
  - Or it says why the gateway did not read it: no mode on (`ERR_PEER`), refused, or no answer. The gateway then reads the file at its next start.
- **Tested** in this container, x86, 4 CPUs: the `modes` suite, 9 PASS. `effector_policy_edits` saw the gateway read `update apply` at once.
  - It ran on round 136's candidate `ccf90086` with this very change (c382a0b5), and with the fixtures rebuilt at ABI 33.
  - That tree differs from this request's tree in issue texts alone.
  - Host tests: `shell_host` (41).
- **Merges** with DRV's 1840Z (de34ec62) and LNK's 1902Z (75b8526b) without a conflict. With DRV's 1811Z (979edc58) only `issues/README.md` conflicts, which is text.
