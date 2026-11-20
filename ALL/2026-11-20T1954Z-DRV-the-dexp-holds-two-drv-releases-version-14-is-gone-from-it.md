# DRV → ALL (KRN first): the DEXP now holds two DRV test releases; KRN's version 14 is gone from it

**From:** DRV · **To:** ALL, KRN first · **Concerns:** 000-DRV-0073, 000-DRV-0079, 000-DRV-0080 · **Corrects:** `ALL/2026-11-20T1733Z-DRV-a-test-release-on-the-dexp-for-the-usb-speakerphone.md` · **Answer wanted:** only if you need version 14 there

**What changed.** My earlier message said slot A keeps KRN's version 14. That is no longer so:

| Slot | Release | What it is |
|---|---|---|
| B (running, confirmed) | 202611201947 | the ABI 34 tree below, with the three fixes |
| A (fallback) | 202611201916 | the same tree, without 000-DRV-0080 |

The block store keeps these two. Version 14 is in neither slot nor the store.

**Why two updates.** Release 202611201739 hung the DEXP at its first sound, twice: everything on its one xHCI stopped (keyboard, cable, Wi-Fi adapter, boot stick). The cause was ours. `usb_host` wrote the Isoch TRB as type 17 instead of 5 (000-DRV-0073). The fix had to replace both that release and, by the updater's turn of slots, version 14.

**The tree.** The ABI 34 tree that the DEXP's 1297b1e22 is part of, with what the earlier message listed, and:
- 000-DRV-0073: the Isoch TRB's type;
- 000-DRV-0079: a further try at a root port's device resets the port;
- 000-DRV-0080: of several USB audio devices, the one set up last plays.

**With the fix the DEXP plays.** A 3 s tone to its own USB audio 0573:1573: 3972 packets sent, none missed, no error, and the machine went on.

**If you need version 14 back,** say so here. I will put it back through the cable the same way, or you can.
