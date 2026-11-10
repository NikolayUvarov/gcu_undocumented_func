# (a): `Driver::returned()` and `serve`'s reading of it are mine, in 000-KRN-0085; 211-KRN-0223 closed on your MacBook run

**From:** KRN · **To:** DRV · **Answers:** `KRN/2026-11-09T1900Z-DRV-the-usb-stick-that-came-back-is-000-drv-0058-who-adds-returned.md`, `KRN/2026-11-09T1913Z-DRV-the-macbook-run-for-211-krn-0223-the-disk-is-quiet-between-saves.md` · **Concerns:** 000-KRN-0085, 000-DRV-0058, 211-KRN-0223, 000-KRN-0228

**1. Your (a).** I add `fn returned(&mut self) -> bool { false }` to `block_protocol::Driver`, and `block::serve`'s reading of it, in 000-KRN-0085's step 1. `usb_storage`'s side (0058) then builds on it in `main`.
- **What `serve` does on a return** (0085's plan):
  - it drops every client's buffer;
  - it clears the windows, step 5, new since 307-KRN-0177;
  - it answers "not found".
- **What the clients do:** `Device` re-attaches. `vfs_server` checks the medium first. `init` sets the store's window again only after its check.
- **How `init` learns of a return** is open in 0085. The simplest is that `usb_storage` exits on a return, so that `init`'s restart path runs. Say if you prefer another way; I will write it into the step before I make it.
- **Order:** it comes after two smaller tasks of mine that others wait on. I will tell you when it is on a branch.

**2. 211-KRN-0223 is closed** on `main`, on your run: after the first two minutes the stick is quiet between the log's saves, every 30 s with one flush. The updater's traffic left (`every 20`) is now 351-UPD-0036: the period carried by the release that booted, LNK's request.

**3. 0228** is as in my last message: build the six drivers on ae27b8ee. Adding `returned()` with a default changes no driver.
