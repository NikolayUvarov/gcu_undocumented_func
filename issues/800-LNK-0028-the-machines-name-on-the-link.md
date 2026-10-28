# 800-LNK-0028 — The machine's name in the link's hello and on `effector485`'s page

**Type:** service, tool · **Owner:** `LNK` · **Priority:** P2 · **Status:** open (2026-10-28) · **Blocked by:** `APP` and `KRN`: the machine's name as a system setting and its reader (750-EFF-0018's proposal, `ALL/`, 2026-10-28) · **Main task:** [800](800-mind-link.md) · **Constitution:** MC-10.2, MC-12.4

## Problem

The maintainer (2026-10-28) wants the machine's name set on the machine itself, not only the Effector agent's.

- **On the link,** every MIND Core machine answers a hello as "MIND Core link".
- **Several machines may share one cable** (800-LNK-0026). Then `effector485` tells them apart by address alone: `link`, `link-5`.

## Plan

1. **`link`'s hello** answers with the machine's name, "MIND Core link on <name>", read through the reader `KRN` chooses.
   - The name rides in the hello's text, so the protocol does not change.
   - Before a name is set, the answer is as now.
2. **`mindlink hello`** prints it.
3. **`effector485`:**
   - each endpoint's line in `endpoints` and on its page shows the name the last hello gave;
   - `agent_id` stays `link`/`link-<n>`, so nothing that addresses a machine changes.
4. **The tests:**
   - `mindlink`'s and `effector485`'s with a named slave;
   - the `link` QEMU suite: `hostname` set through the link's shell client, then the next hello carries it.

## Acceptance criteria

- [ ] Host tests and the `link` QEMU suite as above.
- [ ] On the DEXP: `effector485 endpoints` shows the name set by `hostname`.

## Related

`750-EFF-0018` (the agent's side), [800-LNK-0026](800-LNK-0026-several-machines-on-one-line.md).
