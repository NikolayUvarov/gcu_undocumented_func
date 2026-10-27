# 750-EFF-0017 — An identity of the machine's own: an agent copied with a disk's settings to another machine gets a new ID

**Type:** session · **Owner:** `EFF` · **Priority:** P1 · **Status:** in progress (2026-10-27) · **Blocked by:** — · **Main task:** [750](750-effector-agent.md) · **Constitution:** MC-10.2, MC-12.1, MC-12.3

## Problem

The maintainer (2026-10-27): the DEXP desktop's agent appeared on the server as the MacBook's, `mind_353abc18de2dbd0e`, hostname `mind-macbook-pro`, already activated. "It calls it mbp, but that is wrong."

- **How the identity came over:** the DEXP's new stick was written with `--keep-settings`, which restores the last saved `data/effector/` and `system/effector/`. Those were the MacBook's.
- **Nothing in them names the machine.** The agent ID in `system/effector/agent_id`, the owner's `data/effector/agent_id` and the hostname in `effector.conf` all went to the DEXP as they were.
- **The result:** two machines claimed one agent, the server's owner never saw a new agent to activate, and its alias and history mixed two machines.

## Plan

1. **A machine record** beside the kept identity, `system/effector/machine`:
   - **its first line is the machine's print:** the processor's name (CPUID) and the memory in GiB rounded to a power of two, from the gateway's sample, for example `Intel(R) N100, 16 GiB`;
   - **its second line, if any,** is the hostname the owner's configuration gave the machine the identity came from.
2. **At each start of the service:**
   - **no record**, from an agent before this one: the print is recorded, and the identity is kept;
   - **the same print:** kept;
   - **another print:** the identity was copied. Its agent ID, journal (set aside as `journal.foreign`) and count are put away. The owner's `data/effector/agent_id` is not taken while it names that ID. A new agent ID is made, which the server's owner activates. The record becomes this machine's, with the configuration's hostname as the foreign one;
   - **while `effector.conf`'s hostname is that foreign one,** the agent reports a hostname of this machine's own: `mind-` and the processor's name without maker, marks and clock (`mind-n100`). The log says the owner may name the machine there.
3. **No print** (no gateway sample, no processor name and no memory): the identity is kept as it is, so a failing gateway never makes a new agent.

## Progress

- **2026-10-27:** written.
  - `effector/src/config.rs`: `print`, `machine`, `record`, `own_hostname`;
  - host tests in `tests/effector_config_host.rs`;
  - `effector/src/main.rs`: `machine()` before the agent ID is chosen.
- **On the DEXP, a one-off build with the same code** treated the record's absence as another machine's. Without that, the MacBook's copied identity would have been kept, since no record existed yet. The build was put in over the cable (`effector485` `replace`) and logged:
  - `THE UNRECORDED IDENTITY mind_353abc18de2dbd0e TAKEN AS ANOTHER MACHINE'S`;
  - `HOSTNAME mind-n100`;
  - `NEW AGENT ID mind_09dca8b4def0f6e6`.

  The server lists it online, not activated, hostname `mind-n100`; the MacBook's entry is offline. The one-off is not committed.
- **Until a release carries this agent,** a restart of the DEXP runs the boot image's agent. It keeps the new ID from `system/effector/agent_id` but reports `effector.conf`'s hostname, `mind-macbook-pro`.

## Acceptance criteria

- [x] Host tests: the print and its rounding, the record read and written, a copied identity told apart, the hostname of the machine's own.
- [ ] The `effector` QEMU suite passes, x86 and aarch64: a first start records the machine, and a record of another print gives a new agent ID.
- [ ] On the DEXP, from a release: the record is read as this machine's, and the hostname stays `mind-n100` until the owner names it. Evidence for the DEXP only (MC-12.1).

## Related

[750-EFF-0014](750-EFF-0014-telemetry.md) (the processor's name, the gateway's sample), `800-LNK-0024` (`--keep-settings`, the link track).
