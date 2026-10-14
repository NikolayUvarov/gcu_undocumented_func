# 000-KRN-0109 — The shell's card and TLS clients may badge: a read-only card client and TLS without the device certificate for programs

**Type:** `init`, ABI (`common/abi.rs`, ABI 10, with 550-KRN-0097) · **Owner:** kernel session · **Priority:** P2 · **Status:** in progress (made on the branch; the gate left) · **Blocked by:** — · **Main task:** — · **Constitution:** MC-2.3, MC-3.4, MC-3.11

## Problem

Two network-track requests in `requests-KRN.md`:

- **"A read-only client of a network card for a program"** (000-NET-0015, for the tools track's 000-APP-0049):
  - `idl/net.wit` 1.3 gives a card's driver a client that only reads: with `BADGE_COUNTERS` it may call `info`, `counters` and `offloads`;
  - nobody who would lend it to a program can mint it, because the shell's `SLOT_NET` copy of `virtio_net` has no `CAP_BADGE`.
- **"The right to badge the shell's TLS client"** (351-NET-0022):
  - `tls` refuses the device certificate to a client badged `BADGE_TLS_NO_DEVICE`;
  - the shell lends programs its own unbadged copy, and cannot make the badged one.

## Plan

1. **The badge right.** `init` gives the shell's `SLOT_NET` and `SLOT_TLS` copies `CAP_BADGE` (000-KRN-0073).
   - Both services read their badge only as a restriction: `virtio_net`'s `BADGE_COUNTERS` bit allows reading alone, and `tls`'s `BADGE_TLS_NO_DEVICE` bit withholds the device certificate.
   - So the right lets the shell narrow these clients, never widen them.
2. **`REQUEST_NET_COUNTERS`** (`1 << 23`) in `mind::process`. `SLOT_NET` becomes a launch slot, so the shell can lend the read-only copy in it. `LAUNCH_SLOTS` has 19 slots, and a session's grants (000-KRN-0101) follow.
3. **The shell's part is the tools track's** (requests-APP.md):
   - minting `BADGE_COUNTERS` from `SLOT_NET` for `REQUEST_NET_COUNTERS`;
   - minting `BADGE_TLS_NO_DEVICE` from `SLOT_TLS` for each program it lends TLS to.
4. **The updater's** `tls` client stays unbadged: whether the update server asks for the device's certificate is the update track's call.

## Acceptance criteria

- [ ] The `net` suite (x86 and aarch64): `stat caps` shows the shell's slots 16 and 20 with rights 22 (write, grant, badge) (`shell_badge_rights_check`).
- [ ] `check`'s `launch: every slot` lends an endpoint in all 19 launch slots, `SLOT_NET` among them.
- The tools track's lending and the network track's checks of what a program can do with each copy follow (000-APP-0049, 351-NET-0022).

## Related

[550-KRN-0097](550-KRN-0097-the-wifi-station-in-the-boot-services.md), 000-KRN-0073 (`CAP_BADGE`), 000-KRN-0101 (a session holds every launch slot), [000-NET-0015](000-NET-0015-a-card-read-only-client.md), [351-NET-0022](351-NET-0022-tls-without-the-device-certificate.md).
