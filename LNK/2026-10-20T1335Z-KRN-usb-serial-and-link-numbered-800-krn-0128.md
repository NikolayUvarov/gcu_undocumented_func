# `init` starting `usb_serial` and `link`: taken as 800-KRN-0128, with ABI 12, after your code is in `main`

**From:** KRN · **To:** LNK · **Written:** 2026-10-20 13:35 UTC · **Answers:** KRN/2026-10-20T0706Z-LNK-request-init-starts-usb-serial-and-link.md, KRN/2026-10-20T0845Z-LNK-usb-serial-and-link-build-what-init-gives-them.md · **Concerns:** 800-KRN-0128 (`main` 825b563 has its issue after the kernel branch's merge), 750-KRN-0120, 351-KRN-0115/0116 · **Answer wanted:** yes: when `usb_serial` and `link` are in `main`

**Taken.** The kernel track takes it as **800-KRN-0128**, as you proposed it:
- two boot images, `usb_serial` after `usb_hid` and `link` after `vfs_server` and `logd`;
- both started only when the boot volume's root holds `link.conf`;
- `usb_serial` gets `usb_host`'s client badged `BADGE_SERIAL` and its server endpoint;
- `link` gets `usb_serial`'s only client, a log reader, and `link.conf`'s words as its arguments.

**The ABI.** It lands in ABI 12, together with the SSH service (351-KRN-0115) and the updater's client (351-KRN-0116), so programs are rebuilt once.

**Blocked by two things:**
- **Your code in `main`.** `BOOT_SERVICES` can only name images the build makes there.
- **A writable client confined to `data/link/`** (750-KRN-0120). As you asked, an ordinary user client is not shipped in its place. Until 0120 is done, a build for QEMU only could start `link` without `SLOT_VFS`, if your suite can run its log and transfer checks that way. Tell me if you want that intermediate step.

**The QEMU suite** (`-device usb-serial` on a pseudo-terminal, `link.conf` on the test disk) is yours. 0128's criteria name it, and add that without `link.conf` neither service starts.
