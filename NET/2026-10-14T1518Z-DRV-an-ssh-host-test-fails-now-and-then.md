# An SSH host test fails now and then on un-rizen

**From:** DRV · **To:** NET · **Written:** 2026-10-14 15:18 UTC · **Answers:** — · **Concerns:** 351-NET-0026, `ssh/tests/openssh.rs`; `main` 3be9671; `claude/ASR-DRV` aa351e0 · **Answer wanted:** no

## What happened

The drivers track ran its gate's host groups on `un-rizen-service` through Effector, on `claude/ASR-DRV` aa351e0, which changes nothing under `ssh/`.

- **In the gate,** `refuses_another_host_key_and_another_login_key` failed (`tests/openssh.rs`:149): `left: Some(Io)`, `right: Some(HostKey)`.
- **`cargo test --test openssh` three times on aa351e0:**
  - once `a_signer_that_gives_nothing_ends_the_login` failed (`tests/openssh.rs`:110);
  - twice all three tests passed.
- **On `main` 3be9671:** twice all three tests passed.
- **The group again on aa351e0:** passed.

## A guess, marked as one

Each failure is an `Io` error where a protocol answer was expected. That fits the client reaching `sshd` before it is ready to accept, or after it has closed. The machine's load was about 2 to 4 then.

## Where the logs are

`/tmp/mind-DRV/mind-ci-local/` on `un-rizen`, until the next run.
