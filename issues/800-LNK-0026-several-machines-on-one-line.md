# 800-LNK-0026 — Several machines on one RS-485 line: the protocol's version 2 with the slave's address

**Type:** protocol, service, tool · **Owner:** `LNK` · **Priority:** P1 · **Status:** in progress (2026-10-26) · **Blocked by:** — · **Main task:** [800](800-mind-link.md) · **Constitution:** MC-10.2, MC-12.1, MC-12.4, MC-12.7

## Problem

The maintainer (2026-10-26) asked whether `effector485` could reach two or three machines on one cable at once, and then said to start: a further converter or two is being bought for the channel.

- **The wire allows it.** Two-wire RS-485 is a bus: one pair carries up to 32 standard loads, and the protocol already has a single sender at a time (stop and wait, the master alone starts an exchange).
- **The protocol does not.** A version 1 frame names no slave. Every slave on a shared pair would answer every request, and their answers would collide.
- **The daemon does not.** `effector485` holds one port for one machine: one queue, one reservation, one state.

## Plan

1. **The protocol's version 2** (done; `docs/link/protocol.md` section 3a): an address byte, 1 to 247, after the version.
   - A slave with an address answers only version 2 frames to it.
   - A slave without one answers only version 1 frames, as every slave before this change does, so old slaves stay silent while a master addresses others. At most one of them may be on a shared line.
   - A line with one slave keeps version 1, so nothing changes for it (MC-12.7: the old version stays served, the new one is explicit).
2. **`linkproto`** (done): `Header.address`, the decoder for both versions, `Master.address` and `Slave.address`. A master takes only its slave's answers; a slave skips requests to others and their answers.
3. **MIND Core's `link`** (done): `address=<n>` in `link.conf`, which reaches `link` through the bootloader and `init` as the other words do (`LINK_WORDS_MAX`, 64 bytes; no ABI change). A bad value is logged and the line is served as a line of one.
4. **`mindlink`** (done): `--address <n>` for the master and for `serve`.
5. **`scripts/build_and_write.sh --link-address=<n>`** (done) writes the word into `link.conf`.
6. **`effector485`** (to do): several machines on its one port.
   - Each machine is an endpoint of its own: its address, queue, reservation, state and log cursor.
   - The worker serves the endpoints' operations in turn, one exchange at a time, so a long `put` to one does not stop the others for its whole length.
   - The keep-warm POLL goes to each.
   - The page and the API show each endpoint, and an operation names the endpoint it is for.
   - With no addresses configured, the daemon is as before.
7. **Tests:**
   - `linkproto/tests/link.rs` (done): three addressed slaves and one without, on a simulated shared line; each address answered by its slave alone, files only on it; the one without reached only without an address; an unknown address times out. Version 2's frame round trip and its limits.
   - `tools/mindlink/tests/bus.rs` (done): the same with four `mindlink serve` processes on a TCP hub that repeats each end's bytes to the others.
   - The QEMU `link` suite: a second boot with `address=5` in `link.conf`; `link` answers address 5 only.
   - `effector485`'s host tests: two endpoints on one hub, operations to each, a reservation on one leaving the other free.
8. **On the maintainer's pair,** once the converters are there: the MacBook Pro at address 1 and a second machine on the same pair, both reached through `effector485` on COM22. Rates and losses are recorded here (MC-12.1).

## Acceptance criteria

- [x] `linkproto` and `mindlink`: a shared line with three addressed slaves and one without, each reached alone, in host tests (evidence for the simulated line only).
- [ ] The QEMU `link` suite: `link` with `address=5` answers only frames to address 5.
- [ ] `effector485`: two or more endpoints on one port, each with its own queue, reservation and state; host tests.
- [ ] Docs: `protocol.md` (version 2), `mindlink.md`, `effector485.md`, in English and Russian.
- [ ] On the maintainer's pair: two machines on one cable through `effector485`, each answering only its own address.

## Related

[800-LNK-0004](800-LNK-0004-mindlink-the-host-tool.md), [800-LNK-0011](800-LNK-0011-effector485.md), [800-LNK-0021](800-LNK-0021-effector485-reservations.md), [800-LNK-0020](800-LNK-0020-the-remote-links-marks-on-the-screen.md).
