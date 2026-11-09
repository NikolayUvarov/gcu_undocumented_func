# Ready: the card a machine's traffic takes, chosen in software (550-NET-0062)

**From:** STO, making 550-NET-0062 for the network track · **To:** LANDING · **Written:** 2026-11-09 12:57 UTC · **Concerns:** 550-NET-0062 (advances: its QEMU criteria met; DRV's Ryzen check follows the landing)

**Ready:** `claude/relaxed-meitner-5bmhpz` d4b16f6b1c6d0f781e177f22f8c4bea9252cb2dc

It is `main` 6dc3fdf4, round 99, with this branch merged in. `main` has moved since, to round 100 (39351fa0, `vfs_server` alone); this branch merges with it without a conflict.

## What it brings

- **`netstack`,** the routing rule:
  - a card's own address first;
  - then, among the cards whose network holds the destination, the preferred one, then the lowest;
  - then the default card, as before.
- **`netstack`,** a card out of use (`withdraw`, `Link::off`):
  - no route through it and no address;
  - its TCP connections aborted and counted; listening sockets stay;
  - its interface not polled, so nothing is sent there, DHCP included;
  - the link is still watched, and DHCP starts again when the card is back.
  - Every card out of use is allowed, and logged with the way back. A `withdraw` from any badge but the policy's and the operator's is refused and logged with its badge.
- **`netpolicy`:** the line `card N off`, told to the stack with the preferred card: at start, after each change and every 10 s.
- **`idl/socket.wit` 2.11** (minor): `withdraw` and `withdrawn`, with the regenerated `libmind/src/idl/socket.rs`.
- **`netcheck`:** `cards` shows `OFF` and every card out of use. `withdraw:MASK` asks as a grant, which is refused.
- **`tests/qemu_smoke.py`:** `one_network_cards_check` in the `net` suite, two cards on one network.
- **Docs and issues:**
  - `docs/network/policy.md` 0.8 and `docs/idl`;
  - the issue, with its request taken out of `requests-NET.md`;
  - a request to APP for `ip`.
- **No change** to the ABI, `init` or any driver.

## Tested, in this session's container (4 CPUs, QEMU 8.2.2 under TCG), on d4b16f6b

- **Built** x86 with `scripts/x86_fixtures.sh`, and aarch64 with `--fixtures`.
- **The `net` suite:** x86 39 checks, aarch64 36, all passed.
  - aarch64 skips 3: the fixtures built for x86 alone (the late attach, the slow radio, the world country), as before this change.
- **The new check** shows every acceptance criterion in QEMU:
  - six datagrams to the network's gateway go through card 0, through card 1 with `default card 1`, and through card 0 again without the line;
  - `card 0 off` drops card 0's address and moves the default route to card 1. Card 0's sent frames do not grow over 12 s, and a grant's `withdraw` is refused and logged with its badge;
  - `card 1 off` too: every card is out of use, and the log says so;
  - both are back, each leased again, and the network's flows return to card 0.
- **The stack's other users,** on aa26af05 and 51db3f24, before `main` was merged in:
  - x86: `tls`, `effector` and the `updater` group, 23 checks;
  - aarch64: `tls` and `effector`, 17.
- **`scripts/host_tests.sh`:** every Rust test passed. One Python test failed: EFF's `effector_server_test.py` `test_contract_tls_pin_auth_and_prefix`, reported to EFF on 2026-11-05, which fails the same way on `main` in this container. This branch touches nothing of Effector.
- Not run on real hardware: DRV runs the Ryzen check after the landing (KRN's word).

It is merged into `fast-test` (9ace2e54).
