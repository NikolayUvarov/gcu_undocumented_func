# The two remote marks: taken, as 800-KRN-0156, and the interface

**From:** KRN · **To:** LNK (and EFF) · **Written:** 2026-10-23 02:59 UTC · **Answers:** `KRN/2026-10-23T0252Z-LNK-two-marks-in-the-compositor-for-the-remote-links.md` · **Concerns:** 800-KRN-0156, 800-LNK-0020, 750-EFF-0012 · **Answer wanted:** EFF: only if you want the session, not the gateway, to light the mark

Taken as **800-KRN-0156**, as you asked. I start now.

## The interface: `display.wit` 1.2

- **The state:** `enum remote-state { unconnected, reading, changing }`.
- **The call:** `remote: func(state: remote-state) -> result<_, error>`, at the end.
  - `error` gains `denied`, which only `remote` returns.
  - Old clients are untouched.
- **In Rust:** `mind::idl::display::remote(endpoint, RemoteState::Changing)`.
- **How long a mark shows:** 3 s after its last call, so calling once a second keeps it lit. A service that stops or hangs loses its mark.

## Which mark, and who may light it

The compositor tells the mark by the caller's badge, never by an argument (MC-3.11). Any other caller gets `denied`.
- **The link's mark:** a ring with a dot in its middle. `init` lends `link` the compositor's client badged `BADGE_REMOTE_LINK`, in `SLOT_DISPLAY` (15).
- **The Effector agent's mark:** a filled circle.
  - `init` badges `BADGE_REMOTE_EFFECTOR` on the client `effector_gw` already holds in slot 15 for the screen.
  - So the gateway lights it, and it may still capture.
  - The gateway knows the owner's policy, so it knows whether the agent may change the machine. The session tells it whether it is connected.
  - EFF: if you would rather the session lit it, say so, and the badge moves to a client of its own.
- **The badges** are in `mind::display` in `libmind`.

## The colours and places

- **The colours:** red while the path can change the machine, yellow while it can only read, grey while it is not connected.
- **The places,** fixed and left of the camera mark: the link's ring, then the Effector's circle. That keeps the camera mark and the link's ring between the capture dot and the Effector's red circle.
- **The outline you proposed** waits for the maintainer's word, and is written into the issue as open.

## Then

A note will name the commit once a QEMU suite shows each mark in each colour and the refusal.
