# 800-LNK-0020 — The screen shows who can reach the machine: the link's mark and the network Effector's

**Type:** service, tool · **Owner:** `LNK` · **Priority:** P1 · **Status:** open (2026-10-23) · **Blocked by:** `KRN`: the compositor's two marks and their interface; `EFF`: the agent's state for its mark (both asked on `internal-dev-messages`, 2026-10-23) · **Main task:** [800](800-mind-link.md) · **Constitution:** MC-2.3, MC-3.11, MC-7.5, MC-10.2, MC-12.6

## Problem

Since 2026-10-23 two remote paths reach the MacBook Pro running MIND Core:
- `effector485` over the serial cable (800-LNK-0017), through `link`;
- the Effector agent over the network (750-EFF-0012).

Nothing on the Mac's screen says that either path is connected, or what it may do there. A person at the keyboard cannot tell whether someone is reading the machine or able to change it.

## The maintainer's word (2026-10-23)

- **The link's mark:** while `effector485` (or any peer) is connected over the link, a mark in the top right corner of the screen:
  - **red** while the peer can change the machine;
  - **yellow** while it can only read.
- **The network Effector's mark:** while the Effector agent runs and is active, a mark beside it, under the same colour rules.
- **The shapes differ, at the same diameter:**
  - the network Effector's is a plain filled circle;
  - the link's is a ring with a dot in its middle.
- **Grey** when the path is not connected.

## Plan

1. **`KRN`, the compositor** (asked): it draws both marks on the framebuffer, as it draws the capture dot and the camera mark. No capture then holds them, and no program can hide them.
   - **The interface:** a new `display.wit` call.
   - **Who may call it:** only `link` and the Effector agent may light their marks. The compositor tells them apart by the caller's grant or badge from `init`, never by what the caller says (MC-3.11).
2. **`LNK`, `link`** reports its state every second while it runs:
   - **grey:** no frame from a peer in the last 5 s;
   - **red:** a peer, with the developer mode (`dev` in `link.conf`). The peer runs programs, shell lines, `replace` and `reboot`, and with `link-console` (800-LNK-0019) the console's commands;
   - **yellow:** a peer, without it. The peer reads the log, and files go only into and out of `data/link/`.
3. **`LNK`, `effector485`** keeps the line warm: while it holds the port and the line is up, one small frame every 2 s, so the mark stays lit between operations.
4. **`EFF`, the agent** (asked) reports its state in the same way:
   - **grey:** not connected to its server, or not activated;
   - **red:** the owner's policy grants something that changes the machine (delivering files, controlling services, running, updating);
   - **yellow:** it grants only reading (the screen, logs, collecting files).
5. **Tests and documents:**
   - the QEMU link suite checks the link's state calls and its colours, with `dev` and without, and grey after the peer goes silent;
   - `docs/link/` says what the mark means.

## Acceptance criteria

- [ ] With `effector485` connected to a machine with `dev`, the ring with a dot is red. Without `dev`, it is yellow. 5 s after the cable is pulled or the daemon stops, it is grey.
- [ ] With the Effector agent connected and active, the filled circle beside the ring is red or yellow by the owner's policy. With no server, it is grey (`EFF`'s part).
- [ ] No other program can light or put out either mark (`KRN`'s check).
- [ ] The QEMU link suite checks the link's side, and the MacBook Pro shows both marks (evidence for that machine only, MC-12.1).

## Open point for the maintainer

The capture dot (issue 165) is already a red filled circle in the top right corner, meaning "the screen is being captured". The network Effector's red mark is also a red filled circle. `KRN` is asked to keep the two apart, by their places and by anything else the maintainer agrees to, such as an outline on the Effector's mark.

## Progress

- **2026-10-23, the compositor's marks are made** (800-KRN-0156, landed in round 18):
  - `display.wit` 1.2 `remote(state)`;
  - `link` holds the compositor's client in slot 15, badged `BADGE_REMOTE_LINK`;
  - the ring with a dot sits at (width − 72, 18), the Effector agent's filled circle at (width − 96, 18);
  - each shows 3 s past the last call.
- **2026-10-23, `link` lights its mark.** Every second it calls `remote()`:
  - `Changing` with `dev`, `Reading` without it;
  - `Unconnected` once no request has come for 5 s;
  - it logs each change as `[LINK] MARK: …`. With no grant, as with an older `init`, it says so once and stops asking.
- **2026-10-23, `effector485` keeps the line warm.** A link-protocol POLL goes out every 2 s while the daemon is idle. The other end does not log it, as it logs a hello. The console's traffic line ignores moves of up to 512 bytes in 5 s.
- **The QEMU `link` suite checks it.** The suite runs with `dev`:
  - `[LINK] MARK: Changing` and the compositor's `REMOTE MARK Link: Changing`;
  - then `Unconnected` on both after 8 s of quiet;
  - then `Changing` again at the next request.
- **`docs/link/mindlink.md`** and its Russian version say what the mark means.
- **Still open:**
  - the MacBook, which needs a stick or a slot with KRN's round 18;
  - the Effector agent's side (`EFF`'s 750-EFF-0015);
  - the maintainer's choice on telling the red Effector mark from the red capture dot.

## Related

[800-LNK-0017](800-LNK-0017-effector485-page.md), [800-LNK-0019](800-LNK-0019-the-link-as-the-owners-console.md), 750-EFF-0012, [165](../issues-done/165-display-client-for-programs.done), [158](158-video-capture.md).
