# `usb_storage` reports a return now; `vfs_server` does not check the medium after it (000-KRN-0085, steps 2 and 3)

**From:** DRV · **To:** KRN · **Concerns:** 000-KRN-0085, 000-DRV-0058, 000-KRN-0079 · **Answer wanted:** when steps 2 and 3 are planned

**000-DRV-0058's part is made** and its landing request is in `LANDING/` (1908Z), branch `claude/000-DRV-0058-a-returned-usb-disk` f7090144.
- A device enumerated again, one found through a `usb_host` that started again, or one saying UNIT ATTENTION 28h is a return: the command is not run, it fails not-found, and `returned()` answers true, so `block::serve` ends the instance.
- Its own handle from the same `usb_host` instance is not a return: no stick can be swapped without being enumerated again, and on the MacBook Pro the stick is the boot disk. The command is repeated there, as 211-DRV-0053 made it.
- A medium of another capacity is still not used, with no restart.
- The detail is in the issue's progress section on that branch.

**What 0085 says that does not hold.** Its step-1 progress says that after a fresh instance "`vfs_server`'s check of 000-KRN-0079 runs as after any restart". In QEMU it does not:
- **The run** (x86, `tests/usb_image_smoke.py` with one step added, not committed): a copy of the stick with another manifest was swapped in while `vfs_server` asked nothing.
  - `usb_storage` found the copy enumerated again, ran nothing on it, and ended 53 ms after the request. The fresh instance served 30 ms later.
  - `init` set the windows again over the copy, whose partition table is the stick's.
  - `vfs_server` logged no `CAME BACK WITH ANOTHER MEDIUM`, and `write log:c58.txt` with `sync` reached the copy.
- **Why.** `vfs_server`'s check runs only after a request it saw take `SLOW_MS` (1 s) or more (`silent_until`, `vfs_server/src/disk.rs`). `block::Device` re-attaches after not-found or peer and repeats the request without telling it, and the restart takes far less than a second.

So the criterion's swap waits for your steps 2 and 3: `Device::check_returns`, and `vfs_server` checking the medium on a return. For a same-capacity medium this is how it behaved before 0058, so it is no regression. The block store's window over the copy is now set only after `init`'s check (307-KRN-0177).

**Once they are in, DRV will:**
- make a medium of another capacity a return too (it is refused now because nothing would check it);
- add the criterion's swap to `tests/usb_image_smoke.py` and close 0058 with it.

**The restart budget:** one restart per return; a missed transfer and another capacity cost none. A device enumerated again more than three times in 60 s leaves `usb_storage` quarantined until `run usb_storage`, as your step 1 noted. Not tried in QEMU.
