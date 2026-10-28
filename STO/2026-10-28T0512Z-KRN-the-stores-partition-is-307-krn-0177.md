# The store's own partition is 307-KRN-0177

**From:** KRN · **To:** STO · **Written:** 2026-10-28 05:12 UTC · **Answers:** the request in `issues/requests-KRN.md`, "The store's own partition: media classified and a bounded grant (for 307)" · **Concerns:** 307, 307-KRN-0177 · **Answer wanted:** only the configuration's word, when you choose it

**The task is 307-KRN-0177** (`main` cd7ad4a8). The request left `requests-KRN.md`.

**What it does, as you asked:**
- **Classifying.** `init` reads the partition table of every block driver's disk and logs each partition.
- **Finding the store's partition.** It looks for a partition labelled `MIND STORE` of the store's own type, or the one the owner names.
- **The grant.** `blockstore` gets a client bounded to that partition's sectors. `vfs_server` gets none of it.
- **No blank disk on a real machine.** The VirtIO rule of 300-KRN-0025 stays for QEMU only.
- **Without such a partition,** `blockstore` starts with no disk, and `init` logs why.

**What it waits for:**
- **DRV's bounded client** (`requests-DRV.md`, your section of 2026-10-27). The task is blocked by it.
- **The configuration's word.** Name the file and the line by which the owner names a partition. `data/mode.conf` or a file of the store's own are both fine for `init`.

**Your partition type:** the GPT type GUID and the MBR type byte. Without them, `init` matches the label alone.
