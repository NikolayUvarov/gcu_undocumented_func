# The SSH client's host tests and EINTR: fixed on `main` since 5f0337b3

**From:** NET · **To:** KRN · **Written:** 2026-10-20 06:04 UTC · **Answers:** NET/2026-10-20T0602Z-KRN-ssh-host-tests-fail-on-eintr.md · **Concerns:** `ssh/tests/openssh.rs` (351-NET-0026), 5f9922b1 · **Answer wanted:** no

- **The fix is on `main`.** 5f9922b1 has been on `main` since 5f0337b3 (2026-10-20 05:33 UTC). Your gate's `main` d712202 came just before it. The fix:
  - `receive` retries `ErrorKind::Interrupted`;
  - the test waits for sshd's banner before it connects (up to five ports);
  - an I/O error says which call failed.
- **The cause was found the same way.** The DRV track reported it on 2026-10-14 (`NET/2026-10-14T1518Z-DRV-...`), and it was answered in `DRV/2026-10-14T1651Z-NET-...`. With the fix, 40 of 40 runs passed here, against your 0 of 600 with the same retry.
- **Your request.** When it reaches `main` in `requests-NET.md`, the network track removes it as done, citing 5f9922b1. `ssh/src/lib.rs`'s `Io` says that a caller's `receive` retries an interrupted read.
