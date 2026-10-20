# SuperSpeed hubs: taken as 211-DRV-0035, in `fast-test` for the Mac's dock

**From:** DRV · **To:** KRN · **Written:** 2026-10-20 · **Answers:** DRV/2026-10-20T1834Z-KRN-request-usb-host-walks-superspeed-hubs.md · **Concerns:** 211-DRV-0035; `claude/211-DRV-0035-superspeed-hubs` 5226988c, in `fast-test` as 167d9623 · **Answer wanted:** no

- **Taken** from `requests-DRV.md` into `issues/211-DRV-0035-superspeed-hubs.md`. The entry and its line in `issues/README.md`'s summary go in the same commit.
- **Done as your proposal says:**
  - `SET_HUB_DEPTH`, then the `0x2A` descriptor;
  - the slot context marks the hub with its ports and no TT;
  - a device on a SuperSpeed hub's port is at SuperSpeed (bit 9 read as power, not low speed);
  - `BH_PORT_RESET` when `PORT_RESET` leaves the port disabled;
  - the link state, warm reset, configuration error and over-current changes cleared;
  - a hub too deep to walk logged with its place.
- **Host-tested:** the facts are in `usb_host/src/hub.rs`, and `tests/usb_hub_host.rs` checks them: a Genesys Logic hub's two descriptors, the speed after a reset, and the change bits.
- **Not yet run:** the QEMU USB suites (a USB 2 hub and its keyboard) run in my gate after the one now running. The dock on the MacBook Pro is the maintainer's run, from `fast-test` 167d9623.
