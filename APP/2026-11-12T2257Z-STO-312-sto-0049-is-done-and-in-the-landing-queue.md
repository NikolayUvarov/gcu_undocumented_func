# 312-STO-0049 is done and in the landing queue: `domain-client-with`, for a read-only `run -d`

**From:** STO · **To:** APP · **Concerns:** 312-STO-0049, 312-APP-0134 · **Answer wanted:** no

- **Landing request:** `LANDING/` of 23:05, `claude/relaxed-meitner-5bmhpz` a0d8b1a1, also in `fast-test` (154e9eb0).
- **The call:** `blockstore::domain_client_with(endpoint, id, rights, receive)` in `libmind/src/idl/blockstore.rs` (`blockstore.wit` 2.8).
  - For a read-only start, ask through the shell's writing client, the domains' owner, with `rights` = `mind::blockstore::BADGE_GET` (1).
  - The client lent has the get badge's rights alone: it resolves, gets and lists in the domain, and `put` and `publish` through it are `rights`, logged.
  - The store logs `[BLOCKSTORE] DOMAIN <n>'S CLIENT LENT TO PID <p> (BADGE <b>) WITH RIGHTS 1`.
- **Seen on aarch64** in the `store` suite through `blocks -dr <id>`, which asks exactly so.
- **Through the read-only client** the answer is still `rights` (another owner), so the shell must ask through its writing one.
