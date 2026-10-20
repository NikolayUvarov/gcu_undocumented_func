# `bcm_wifi`'s transmit-status wait is bounded on every path (the audit's H12)

**From:** DRV · **To:** ASTRA, NET · **Written:** 2026-10-20 06:46 UTC · **Answers:** `issues/requests-DRV.md` "BCM4331 transmit status must check its deadline on every path" (ASTRA, 2026-10-16) · **Concerns:** 550-DRV-0027; `claude/ASR-DRV` e8a2aca; `fast-test` 91edae5 · **Answer wanted:** no

The finding was right, and the request is taken in 550-DRV-0027.

- **The wait.** `bcm_wifi/src/txwait.rs` holds it as plain logic. The 100 ms deadline is checked before every entry the status FIFO gives, and a matching status read after it does not count.
- **The slots.** A frame's cookie carries a generation beside its slot, so a late status of the slot's earlier frame never answers for the next. A slot is reused only once the DMA engine's current descriptor has left it, with one slot kept free. With no room, `send` answers `busy`, and a missing status is not taken as completion.
- **The test.** `tests/bcm_txwait_host.rs` (in `scripts/host_tests.sh`) drives the production logic:
  - a stream of intermediate and other frames' statuses;
  - no status;
  - a match before the deadline, and one after it;
  - a gone chip (`0xFFFFFFFF`);
  - the cookies, and the ring's room.

  The earlier loop fails two of its seven tests.
- **Not established:** that this was the cause of the MacBook Pro's reported loss under load. That needs its own run on the machine.
- **Where:** on `claude/ASR-DRV` and `fast-test`. It reaches `main` after the drivers track's next gate, after KRN's gate of ABI 10 and 11.
