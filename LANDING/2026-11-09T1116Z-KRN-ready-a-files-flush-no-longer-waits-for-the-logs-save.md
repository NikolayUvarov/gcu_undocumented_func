# KRN ready: a file's flush no longer waits for the system log's save (211-KRN-0223)

**Ready:** `claude/youthful-mendel-mf1soy` 9c79dcc3

**From:** the kernel session (KRN)

**Advances:** 211-KRN-0223, the cause of round 99's first failure of `aarch64: network above 4 GiB`.
- **What was wrong since 0223:** `vfs_server` answered every `flush` by first saving the system log's journal and flushing the log volume.
- **Who waited:** the network policy broker's store of a new line ends with the file's flush, inside the shell's 1 s for an answer. So `ALWAYS` came back `THE BROKER DID NOT ANSWER` once.
- **Now:** only a volume root's flush does that (the shell's `sync`, `init`'s flush before a restart). A file's flush writes its own volume only, as before 0223.

**The change:** `vfs_server/src/main.rs` (7 lines) and the issue's record. It is `main` c92256a3, then 6dc3fdf4's text, then this commit.

**Tested** in the kernel session's container (QEMU 8.2.2, TCG), on 9c79dcc3:
- `x86: boot, display, shell, memory, clock` (the `boot` suite's log check);
- `x86: USB image` (the `sync` before the medium swap, the log read on the host);
- `aarch64: network above 4 GiB` (the `net` suite's `browse_check`);
- all passed.
