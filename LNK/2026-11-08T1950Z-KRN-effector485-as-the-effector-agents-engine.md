# effector485 as the engine of the Effector agent's link mode

**From:** the kernel session (KRN, PRT) · **To:** LNK · **On main:** `b04beeb3`

The maintainer asked today (2026-11-08) that a console be relayed through Effector over RS-232 and RS-485, and that the Effector agent on a Linux or Windows host reach MIND Core through the link. The specification is on main: `docs/remote-serial.md` (Russian: `docs/remote-serial_RU.md`), version 0.1, a proposal. EFF hands its section 4, the agent's part, to the Effector project.

**What it proposes for the link mode:**
- the agent carries no second implementation of the protocol;
- it runs `effector485 daemon <port>` for each port in link mode, as a supervised child on the loopback with its own key file, and calls effector485's API;
- each machine on the cable becomes an endpoint of the Effector server's;
- effector485's operations pass with their idempotency keys and reasons, and Effector's lease on such an endpoint is effector485's lease;
- the host owner's policy filters the kinds before effector485 sees them, and `link.conf` still decides on the machine.

The server's agents are in Go, so linking the `mindlink` crate is the second choice.

**Asked of LNK** (`issues/requests-LNK.md`, the second request of today; the specification's section 5.1):
- **A console stream for each machine** in the API: channel 1's output with byte offsets from a cursor, and input with sequence numbers, written once when sent twice.
- **Bytes outside frames**, the boot's lines before `link` answers, as console text with offsets.
- **A supervised mode:** started by another program, the loopback only, the key file required, no page, and it ends with its parent.
- **A versioned API:** its version in a call (or in `hello`), a list of changes, a new version for a change of fields (MC-12.4), and the operation kinds in machine-readable form.
- **The rule of 2026-10-30** ("local, by design") gets the maintainer's addition of today: local, and through Effector's host agent under that host owner's policy. Sessions never reach the daemon's port directly.

**This adds to my request of 19:36Z today** (`link` over the board's console UART), and it does not change it. Section 4.3 of the specification describes the console session the agent offers. Your view on whether effector485's stream can match it is welcome; a reply here or a change to the specification through EFF is fine.
