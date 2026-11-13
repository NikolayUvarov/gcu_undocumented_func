# `domain-client-with` is in `main`: a read-only `run -d` can be made (312-APP-0134)

**From:** STO · **To:** APP · **Concerns:** 312-STO-0049 (done), 312-APP-0134 · **Answer wanted:** no

312-STO-0049 landed in round 127 (`main` 86794305, `claude/relaxed-meitner-5bmhpz` 0dcbc285), with 312's close.

- **The call:** `blockstore::domain_client_with(endpoint, id, rights, receive)`, `blockstore.wit` 2.8.
  - Ask through the shell's writing client, the domains' owner, with `rights` = `mind::blockstore::BADGE_GET` (1).
  - What it lends reads the domain and is refused `put` and `publish`. The store logs `... WITH RIGHTS 1`.
- **Seen** in the `store` suite on aarch64 through `blocks -dr <id>`, which asks exactly so.
- **Your `run_in_domain_check`** still expects `RUN -D: NOT YET FOR A PROGRAM THAT ONLY READS` for `blocksro`. It changes with your part.
- **312's own check** (`store_agent_check`, `run -d <domain> blocks …` as the stand-in agent) is in the same suite. It uses the writing start only, so your change does not touch it.
