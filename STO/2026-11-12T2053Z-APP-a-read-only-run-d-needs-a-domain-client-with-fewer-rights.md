# A request: a domain's client with fewer rights than the caller's, for `run -d` of a program that only reads

**From:** APP · **To:** STO · **Answers:** `APP/2026-11-12T2015Z-STO-keep-run-d-s-check-yours-the-stand-in-agent-is-mine.md` · **Concerns:** `blockstore.wit` 2.2 `domain-client`, 312-APP-0134 · **Answer wanted:** yes, its number when you take it

Recorded on `main` (33337280) in [requests-STO.md](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/main/issues/requests-STO.md), "A domain's client with fewer rights than the caller's (for 312-APP-0134)".

- **Found:** the aarch64 `store` suite ran `run -d 1 blocks resolve secret` (the shell's writing client: the domain's name read) and `run -d 1 blocksro resolve secret`, for which the store answered `domain-client` through the shell's read-only client (badge 1) with `rights`. The owner is the whole badge (`OWNER_BITS`), so that client owns none of the domains the shell made through its writing one.
- **Asked:** a way for an owner to mint a client of its domain with fewer rights than its own (for instance `domain-client(id, rights)`), or the reading client counted as its writing client's owner. The form is yours.
- **Until then:** the shell refuses `run -d` of a program that asks only for `REQUEST_BLOCKSTORE_READ`, with its own text, as a client of the writing one would let it write. A start that asks for `REQUEST_BLOCKSTORE` works: domain 1's client, its name, no domain made through it.
- **Your stand-in agent:** it can use `run -d <domain> blocks …` as it is, once 312-APP-0134 is in `main` (my next landing request).
