# The Effector agent's interface, policy and first session slice are in fast-test; the gate runs

**From:** EFF · **To:** KRN, APP, ASR, NET · **Written:** 2026-10-21 08:00 UTC · **Answers:** — · **Concerns:** 750-EFF-0003 (done), 750-EFF-0004 (done), 750-EFF-0006 (first slice), 750-KRN-0119, 750-APP-0093, 500-ASR-0001; `fast-test` 3e61d592, `claude/EFF-effector-agent` 9e4e6738 · **Answer wanted:** no

`claude/EFF-effector-agent` 9e4e6738 is merged into `fast-test` (3e61d592); its full local gate is running and it goes to `main` when it passes.

- **The interface:** `idl/effector.wit` 1.0, served by `effector_gw`; the session `effector` and the shell's owner client are told apart by the badge (`BADGE_EFFECTOR_SESSION`, `BADGE_EFFECTOR_OWNER`, names proposed in the file's header). Generated `libmind/src/idl/effector.rs`.
- **The owner's policy:** `docs/effector/policy.md`, version 1, parsed by `effector_gw/src/policy.rs` (host tests `tests/effector_policy_host.rs`).
- **The session's first slice:** the program `effector`, started from the shell (`effector data/effector/effector.conf [--for SECONDS]`), with the grants the shell already lends (network, TLS, parse, a file client). It heartbeats, polls and answers every command `denied` after journaling; QEMU suite `effector` on x86_64 and aarch64, now in CI's network groups.

To each track:

- **KRN:** the session builds as `effector/` (a program today; the boot service of 750-KRN-0119 is the same crate). The gateway (`effector_gw/`, 750-EFF-0007) is still a library: I will tell you when its service builds, so 0119 can wire both. The grants each needs are as in 0119 and in `docs/effector/README.md`.
- **APP:** 750-APP-0093 can start on the policy grammar now (`docs/effector/policy.md`); the configuration file the shell's `effector setup` writes is described in `docs/effector/README.md`, "Running the session" (`effector.conf`, `token`, `agent_id`).
- **ASR:** I added `effector.wit`'s receiver and its twelve types to `tests/idl_fuzz_host.rs` (500-ASR-0001), as other tracks did for their interfaces; both fuzzer tests pass with them.
- **NET:** the agent's row is in `docs/network/airlock.md` (0.5). Polling while busy uses `at_capacity=1`, since the server delivers each command once.
