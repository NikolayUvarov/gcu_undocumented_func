# Serial ports through Effector: a part for the Effector project, to hand over

**From:** the kernel session (KRN, PRT) · **To:** EFF · **On main:** `b04beeb3`

The maintainer asked today (2026-11-08) for a remote-control module, so that a console can be relayed through Effector over RS-232 and RS-485, with the Effector agent on a Linux or Windows host reaching MIND Core through MIND Link. EFF explains to the Effector project what to build there, and hands it over.

**The specification** is on main: `docs/remote-serial.md` (Russian: `docs/remote-serial_RU.md`), version 0.1, a proposal. Nothing of it exists yet.

**The request** is recorded in `issues/requests-EFF.md`, the second section ("Serial ports through Effector"). In short:
1. **Section 4 to the Effector project**, as a task of its own there, beside its task 81. It is the agent's serial module:
   - the ports and the host owner's policy (writing to a console, DTR, RTS and break off by default; in link mode only the reading kinds by default);
   - the line, with RS-485's direction (`auto`, or `rts` through `TIOCSRS485` and `RTS_CONTROL_TOGGLE`);
   - the console session: a ring with byte offsets, a capture file, writes with sequence numbers written once, the port's lease, a live stream, the audit;
   - the link mode: `effector485 daemon` as a supervised child on the loopback, one per port, so that the protocol keeps one implementation, LNK's. The server's agents are in Go, so the specification proposes this over linking the Rust crate;
   - safety (MC-7.4 and 7.6 at the agent), Linux and Windows specifics, and tests on pseudo-terminals and com0com, then on cables.
2. **Their answer back here:** the names they choose, how streams travel between the server and the agent, the first agent revision to try, and the host owner's setup steps.
3. **The outcome on main** in an EFF task, and later a section on serial ports in docs/effector.md.
4. **Later, not for the phone:** the same console contract in MIND Core's own `effector`, through `usb_serial`.

**The rest:**
- **LNK's part** (effector485's console stream, its supervised mode and a versioned API) is asked of LNK in `issues/requests-LNK.md`, today.
- **The kernel session's part** is the board's UART: 100000-PRT-0009, PRT-0010 and KRN-0171.

**Why now:** the PinePhone's first test run (100000, stage 0) is to update, collect logs and run tests before the phone has a network. Its only line out is UART0, 3.3 V, 115200 8N1. The specification's section 6 puts the console through Effector first (S1), then the link mode (S2).

The open questions are in its section 7: the default policy, the stream's transport, supervising effector485, and the endpoints' names.
