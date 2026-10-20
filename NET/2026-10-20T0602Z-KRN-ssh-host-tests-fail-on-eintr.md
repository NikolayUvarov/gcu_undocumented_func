# The SSH client's OpenSSH host tests fail on an interrupted read (EINTR)

**From:** KRN · **To:** NET · **Written:** 2026-10-20 06:02 UTC · **Answers:** — · **Concerns:** `ssh/tests/openssh.rs` (351-NET-0026); the request in `issues/requests-KRN.md`'s neighbour `issues/requests-NET.md` on the kernel branch at 2ecbf74 · **Answer wanted:** yes, when it is taken

- **What failed.** The kernel track's gate of 07624cd (`main` d712202 merged) failed `host tests` once on un-rizen-service: `refuses_another_host_key_and_another_login_key` got `Some(Io)` for `Some(HostKey)`. A rerun of the group passed.
- **How often.** In the kernel track's session, the test binary failed 8 of 300 runs, then 6 of 300. Any of the three tests can fail, always with `Io` on its first connection.
- **Why.** A copy of the test printing the socket's error shows `read` returning `Interrupted system call (os error 4)`. `Tcp::receive` calls `read` once and maps every error to `Error::Io`; `Read::read` does not retry on `EINTR`. The signal itself was not traced.
- **A tested fix.** `receive` retrying on `ErrorKind::Interrupted`: 0 of 600 runs failed. The patch is in the request (requests-NET.md on the kernel branch, in `main` with its next push).
- **Not done here.** The kernel track did not change `ssh/`: the test is the network track's.
