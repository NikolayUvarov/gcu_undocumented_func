# 000-ASR-0008 — Select an isolated OpenSSH server for host tests

**Type:** assurance / test infrastructure · **Owner:** ASTRA auditor-fixer (the maintainer's assignment, 2026-10-15) · **Priority:** P1 · **Status:** in progress · **Blocked by:** — · **Roadmap:** Assurance · **Constitution:** MC-12.1, MC-12.3, MC-12.9

## Problem

The SSH host tests and local gate require `/usr/sbin/sshd`. The audit machines have the OpenSSH client and the server's shared-library dependencies, but no system SSH server. An official server package extracted into a temporary directory can run the existing loopback tests as the current unprivileged user; the hardcoded path prevents this without installing a system service.

## Plan

1. Let the SSH host tests select an absolute server path with `MIND_SSHD`, retaining `/usr/sbin/sshd` when the variable is unset or empty.
2. Make the local gate validate the same path as an executable file. Missing or invalid dependencies must still fail; do not skip or weaken a test.
3. Document the test-only package extraction and preserve all existing SSH assertions, temporary keys, loopback binding and process cleanup.
4. Run the real SSH tests with the extracted server and record the package identity and configuration separately from hardware or full-gate evidence.

## Acceptance criteria

- The ordinary installed-server path remains the default, and an explicit absolute path works for both `cargo test` and the local gate.
- Invalid server paths fail clearly; none of the existing SSH tests is skipped or relaxed.
- All existing SSH tests pass against the extracted official server as an unprivileged user, without installing packages, adding accounts, editing system configuration or starting a system service.
- No downloaded package or executable enters the repository. The coordinated gate records its selected test dependency.

## Related

[351-NET-0026](../issues-done/351-NET-0026-the-ssh-protocol.done), [ssh host-test instructions](../ssh/README.md), [local gate](../scripts/ci_local.sh).

## Progress (2026-10-15)

`ssh/tests/openssh.rs` reads `MIND_SSHD`, defaults to the original path, rejects a relative or non-file path and starts the selected executable with the existing arguments. `scripts/ci_local.sh` checks the same default and requires an absolute, executable file. `ssh/README.md` documents unprivileged extraction. The existing cryptographic, login, refusal and SFTP assertions are unchanged.

On Ubuntu 24.04 x86-64, uid 1000, the repository's modified SSH tests pass with `MIND_SSHD=/tmp/astra-sshd-deps/root/usr/sbin/sshd`: two login tests and all three OpenSSH integration tests, including the complete 100,008-byte file transfer. The dependency was downloaded with `apt-get download`, not installed: official Ubuntu `openssh-server` version `1:9.6p1-3ubuntu13.20`, archive SHA-256 `5d0d5eb672ddc27601e2da4e8465c5736dc73375a21bc1cd1834fcb08fb155bf`. Its existing shared-library dependencies resolved locally. No `sshd` account or `/run/sshd` directory exists on this test host; neither was created. The sandbox first denied loopback binding; the successful run used authorized execution outside that sandbox, still as the unprivileged user.

The real gate preflight rejects missing, relative, directory and non-executable paths; an empty variable selects the missing default and fails clearly on this host. A missing selected server makes all three SSH integration tests fail, with none ignored. Shell syntax and `git diff --check` pass. The complete gate remains coordinated separately; these results do not establish hardware or other-host behavior.
