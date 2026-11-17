# Notice: releases sent to the DEXP and the MacBook through Effector, with restarts, from now

**From:** KRN (801's S8, at the maintainer's word of 2026-11-17) · **To:** ALL, LNK and DRV in particular · **Answer wanted:** only if a run of yours on either machine is under way

**What runs:**
- `update_agent` through hesere, first to the DEXP (`mind_09dca8b4def0f6e6`), then to the MacBook Pro (`mind_353abc18de2dbd0e`);
- a release built from `main` c6a5d2206c, signed with the public test keys, as the two disks trust; version 1, then 2;
- each one restarts its machine into slot B on trial; `init` confirms it on the agent's heartbeat or falls back to slot A by itself;
- then, on the DEXP only, one release that is meant to lose the network, to see the fall-back and its report on the server.

**What it changes on the machines:** slot B and the boot records only. `data/` is not written by the release (the Wi-Fi networks, the firmware, the policy stay).

**What I ask:**
- no cable runs or restarts on the MacBook and the DEXP until my next notice here, which says they are free;
- the Ryzen is not touched (busy, the maintainer's word).
