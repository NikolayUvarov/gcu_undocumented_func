# 100004-EFF-0030 — MIND Core as a serial host: `effector` offers `serialhost`'s lines to the Effector server (stage S4)

**Type:** session, gateway · **Owner:** `EFF` · **Priority:** P2 · **Status:** open (2026-11-14) · **Blocked by:** `LNK`: `idl/serialhost.wit` 1.0 (800-LNK-0042); the Effector project: its agent frames as a versioned contract (its task 109) · **Main task:** [100004](100004-two-allwinner-boards-before-the-phone.md) · **Constitution:** MC-7.1, MC-7.4, MC-7.5, MC-7.6, MC-12.1

**Asked by:** MOB and LNK, 2026-11-10 (`EFF/2026-11-10T0549Z-MOB-mind-core-as-a-serial-host-is-asked-for-now-stage-s4.md`), passing on the maintainer's word of that day:
- serial work through Effector on MIND Core, the same as on Windows and Linux;
- RS-485 and RS-232;
- master and slave.

## Plan

1. **The same API as the other agents.** The server's `/api/v1/serial/*` and the agent's frames on `POST /api/commands/serial` (sst-test-deploy `docs/serial-ports.md`, task 108). A session tells a MIND Core host's line from a Windows one's only by the host's name.
2. **Through `serialhost`** (LNK's 800-LNK-0042), per line: its identity and presence, the lease, the ring read from an offset with `gap`, input with `seq` and `pace_ms`, the controls, and link mode with effector485's kinds and channel 1 as the console.
3. **The owner's policy.**
   - A section for serial lines in `data/effector/policy`. By default (docs/remote-serial.md 4.1) console writing, DTR, RTS and break are off, and link mode allows only the reading kinds.
   - Link mode refuses break, DTR, RTS and line changes (0.2).
4. **The gateway** decides each request by that policy and audits it, as for every other action.
5. **Tests:**
   - the `effector` suite against the fixture, with a pseudo-terminal line through `usb_serial`'s stand-in;
   - then the two boards of 100004 on the DEXP, the Ryzen or the MacBook Pro.

## Acceptance criteria

- [ ] The fixture's serial frames, console and link, in the `effector` suite on x86 and aarch64.
- [ ] One of 100004's boards seen live through hesere from a MIND Core host (MC-12.1).

## Related

[docs/remote-serial.md](../docs/remote-serial.md) (section 5.2, stage S4), [100000-EFF-0026](100000-EFF-0026-serial-ports-through-effector.md), [800-LNK-0042](800-LNK-0042-mind-core-as-the-master-of-a-serial-line.md).
