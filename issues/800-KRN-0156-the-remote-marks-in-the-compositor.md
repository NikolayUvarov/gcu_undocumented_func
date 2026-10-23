# 800-KRN-0156 — Two marks in the compositor: who can reach the machine from outside

**Type:** `compositor`, `init`, IDL (`display.wit` 1.2) · **Owner:** kernel session · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** — (for 800-LNK-0020 and the Effector agent) · **Constitution:** MC-3.11, MC-10.2

From LNK's request of 2026-10-23 (`KRN/2026-10-23T0252Z-LNK-two-marks-in-the-compositor-for-the-remote-links.md` on `internal-dev-messages`), on the maintainer's word: the screen must show who can reach the machine remotely, and what they may do there.

## Problem

- **What the screen shows today.** The compositor draws the capture dot (issue 165) and the camera mark (158) on the framebuffer, where no capture holds them and no program can hide them.
- **What it does not show.** Whether the link (`effector485`, or any peer over MIND Link) or the network Effector agent can reach the machine, and whether it may change it.

## Plan

1. **Two marks of the dot's diameter, in fixed places in the top right corner,** left of the camera mark:
   - **the link's:** a ring with a dot in its middle;
   - **the network Effector agent's:** a filled circle.
2. **Their colour:**
   - **red** while that path can change the machine;
   - **yellow** while it can only read;
   - **grey** while it is not connected.
3. **`display.wit` 1.2**, a minor version:
   - `enum remote-state { unconnected, reading, changing }`;
   - `remote: func(state: remote-state) -> result<_, error>` at the end;
   - `error` gains `denied`, which only `remote` returns.
4. **How long a mark shows.** For 3 s after its last call, as the camera mark's rule. So a service that stops or hangs loses its mark.
5. **Who may light which mark.** The compositor tells the mark from the caller's badge, never from an argument (MC-3.11). Any other caller gets `denied`.
   - **`link`:** `init` lends it the compositor's client badged `BADGE_REMOTE_LINK` in `SLOT_DISPLAY` (15).
   - **The Effector agent:** `init` badges `BADGE_REMOTE_EFFECTOR` on the client `effector_gw` already holds in slot 15 for the screen (750-KRN-0153). The gateway lights the mark; the session tells it whether it is connected.
   - **The shell's and the video gateway's clients** stay unbadged: they still capture, and light the camera mark.
6. **Open, for the maintainer:**
   - the capture dot is a red filled circle too;
   - the fixed places tell them apart;
   - an outline on the Effector's mark (LNK's proposal) waits for the maintainer's word.

## Acceptance criteria

- [ ] A QEMU suite (x86 and aarch64):
  - the link's and the Effector's marks drawn in their places in each colour, while called, and gone 3 s after the last call;
  - an unbadged client's `remote` refused.
- [ ] `docs/idl` lists `display.wit` 1.2, and `docs/profile/bootstrap.md` the two grants.

## Related

800-LNK-0020 (on `claude/LNK-link`), 750-EFF-0012, issue 165 (the capture dot), issue 158 (the camera mark), [750-KRN-0153](../issues-done/750-KRN-0153-the-gateways-screen-capture.done).
