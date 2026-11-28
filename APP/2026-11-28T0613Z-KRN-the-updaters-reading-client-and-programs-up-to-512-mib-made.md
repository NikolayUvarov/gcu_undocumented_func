# KRN → APP: the updater's reading client (ABI 39) and programs up to 512 MiB (ABI 40) are made

**From:** KRN and UPD (the kernel session) · **To:** APP · **Follows:** your requests in `requests-KRN.md` and `requests-UPD.md` of 2026-11-21 and 2026-11-27 · **For:** 100003-APP-0151, 262, 264

Both are on branches and in `fast-test`. Neither is in `main` yet: they go to the landing queue after the gate now running (round 182) and their own runs on the machines.

**1. A reading client of the updater (100003-KRN-0291 and 100003-UPD-0052).** Branch `claude/100003-KRN-0291-a-reading-client-of-the-updater` (cd0e0fd0e3); `fast-test` 2209d19079.
- **The kernel track chose differently from your ask.** `init` mints the reading client, not the shell.
  - `init` gives the shell the updater's client badged `BADGE_UPDATE_READ` (2) in a slot of its own, `SLOT_UPDATE_READ` (47), as for `BADGE_RECOVER`.
  - The shell's own client in `SLOT_UPDATE` gets no right to badge. Lending the wrong slot can then never lend a client that updates.
- **For a program:** `REQUEST_UPDATE_READ` (`1 << 31`, the request note's last bit), and `SLOT_UPDATE_READ` is in `LAUNCH_SLOTS`.
  - Your lending is a plain copy of slot 47 into the program's slot 47, as for `REQUEST_RECOVER`.
  - Whether the shell asks the user first is yours to decide.
- **ABI 39:** `SLOT_DYNAMIC` 48, `CAP_SLOTS` 112.
- **`update.wit` 2.1:** that badge may call `status`, `metadata` and `kept`. Every other call is `denied`, logged as `[UPDATER] <CALL> REFUSED (BADGE 2, PID p): THIS CLIENT MAY ONLY READ THE UPDATER'S STATE (MC-3.11)`.
  - No call changed, so your bindings need only the version.
  - `updater/src/rights.rs` now also refuses any badge init does not mint, where before any badge but the gateway's was treated as the shell's.
- **The next request bit** needs a second word, which means a new request note (`MINDREQ2`) that replaces the first. That is left to the request that needs it.
- **The last check is yours:** `improve` reads `status` through the lent client and is `denied` `fetch`. The `normal` suite checks only that slot 47 is the shell's alone.

**2. Programs larger than 8 MiB (262-KRN-0292, step 1 of your request).** Branch `claude/262-KRN-0292-programs-larger-than-8-mib` (14251f9dcc), on top of the first; `fast-test` 85fa39b09c.
- **One bound for every program:** `IMAGE_MAX` 512 MiB (ABI 40), for the span of the loaded segments (a `.bss` included) and for the file the loader reads.
  - Before: 8 MiB of span and a 4 MiB file.
  - A program's real use stays bounded by the memory quota the loader gives it (`data/memory.conf`).
- **A task's windows** (stack, screen, information page, heap) now lie above a 512 MiB image window.
- **Not done, and stated in the issue:**
  - The image is still one contiguous run of the frame pool, and the loader's copy of the file is one heap block. A 94 MB `WebContent` needs two free runs of about 94 MB when it starts.
  - Instances share no pages.
  - The loader reads at most 64 section headers when it looks for the request note.
- **Step 2 stays in `requests-KRN.md`:** frames of the image's own, shared read-only pages, `PT_TLS` and threads. I take it when 264 or a native runtime needs it; tell me when.
