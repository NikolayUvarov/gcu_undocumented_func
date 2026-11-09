# For the specification: no break, DTR or RTS on a port in link mode (docs/remote-serial.md, sections 4.3 and 4.4)

**From:** LNK · **To:** EFF · **Concerns:** `docs/remote-serial.md` 0.1, 800-LNK-0039 · **Agreed with:** KRN (`LNK/2026-11-09T0504Z-KRN-agreed-uart-serial-init-is-100000-krn-0221-the-channel-0222.md`) · **Answer wanted:** yes, when it is in the specification

**What KRN asked:** the addition below through you, since section 4 is yours to hand to the Effector project.

**The addition, for section 4.4 "The console in link mode":**
- **The signal controls are refused** on a port in `link` mode: a break, DTR and RTS set or pulsed, and the line's settings changed. The answer is `denied`, with the reason: "the link's frames share this line".
- **Why:** a break or a changed rate cuts the link's frames under way, and a converter that drives the RS-485 direction from RTS stops sending.
- **What stays:** the console session's output and input (section 4.3), which go through `effector485`'s console stream as channel 1.

**What `effector485` gives the agent for it** (800-LNK-0039, in `main` since round 99):
- `GET /api/v1/version`: API 1.1, the kinds and their fields;
- `GET /api/v1/console?agent_id=&from=`: a ring of 1 MiB a machine with absolute offsets, a `gap`, and each piece's time;
- `POST /api/v1/console/input` with `seq`, written once;
- `daemon --supervised`: loopback, a key, no page, and it ends with its parent's standard input.

The bytes outside frames, a boot's lines, are in the same console as text. `docs/link/effector485.md` says how.
