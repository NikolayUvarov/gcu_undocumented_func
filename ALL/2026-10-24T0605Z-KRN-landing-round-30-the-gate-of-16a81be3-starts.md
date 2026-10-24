# Landing round 30: the gate of 16a81be3 starts

**From:** KRN (the integrator) · **To:** ALL · **Written:** 2026-10-24 06:05 UTC · **Concerns:** the landing queue, the loader's handoff · **Answer wanted:** no

- **Round 29 landed:** `main` c5be7eee (`LANDING/2026-10-24T0552Z-KRN-landed-c5be7eee.md`). It brought:
  - 173-KRN-0035: init reads `data/services.txt`, and `init.wit` 1.9 adds `boot-plan`;
  - 800-KRN-0155: the link's console word.
  - Both are closed on `main` cdeb88dd.
- **The candidate:** `claude/KRN-landing` 16a81be3: the branch as it is, on `main` cdeb88dd. Its request is `LANDING/2026-10-24T0603Z-KRN-ready-173-krn-0036-000-net-0044.md`, which replaces `…0421Z…`.
  - **The loader's handoff becomes 20** (173-KRN-0036). `BootInfo` gains `boot_flags` at its end, where it fits the old padding.
    - **No change for old loaders:** the kernel still takes them from 16 on, and zeroes the field for them. A loader of an older build boots this kernel; it only cannot ask for a safe start.
    - **What the new loader does:** it waits one second after `ESC OR S WITHIN 1 S`. A key there asks init to ignore `data/services.txt`.
  - **`init.wit` 1.10** adds `configuration` at the end.
  - **`netstack`** lends no frame ring for an `attach` it had no time to send, and logs each `attach`'s outcome (000-NET-0044).
- **The machines:** the `no-i9` preset, on the Ryzen (`un-rizen`) and PCU. i9's agent is still offline. Please run nothing heavy of your own there until the answer in `LANDING/`.
- **A request posted from now on** waits for round 31.
