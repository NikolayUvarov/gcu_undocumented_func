# Requests for the state and recovery track (STO), not numbered yet

**Owner:** state and recovery track · **Status:** open (1 request waiting to be numbered: a domain's client with fewer rights than the caller's, APP's of 2026-11-12, for 312-APP-0134); before, 2026-10-29: the note on the block store's `READY` in the aarch64 boot check is settled, since the check waits for it as [000-KRN-0067](../issues-done/000-KRN-0067-the-aarch64-boot-test-on-pcu.done) made it, and `tests/aarch64_smoke.py` saw it on 767c1208 (un-rizen, 2026-10-28); the `blocks check` timeout became [300-STO-0016](../issues-done/300-STO-0016-a-dag-walked-once.done), the collection check's wait [303-STO-0017](../issues-done/303-STO-0017-the-lease-waited-by-the-guests-clock.done), 2026-10-14; the list of the published names [300-STO-0018](../issues-done/300-STO-0018-the-names-listed.done), 2026-10-20)

The STO track numbers its own tasks (`NNN-STO-MMMM`), so requests from other tracks wait here. It turns each into a task and removes it from this file. The file is kept while empty because other issues link to it; a new request goes below this line.

## A domain's client with fewer rights than the caller's (for 312-APP-0134)

**Asked by** APP on 2026-11-12, for [312-APP-0134](312-APP-0134-a-program-started-in-a-memory-domain.md)'s read-only start (`run -d <domain>` of a program that asks only for `REQUEST_BLOCKSTORE_READ`), which STO's answer of 2026-11-12 leaves to APP.

- **What there is:**
  - `blockstore.wit` 2.2 `domain-client(id)` mints a client of domain `id` "with the caller's rights", for the caller's owner;
  - the owner is the caller's whole badge below `FIRST_DOMAIN_CLIENT` (`OWNER_BITS`, 351-STO-0006), so the shell's writing client and its read-only one (badge 1) are two owners;
  - the shell makes its domains through its writing client (`blocks domain new` from the shell, `run -d`), so `domain-client` through its read-only client answers `rights` (seen in the aarch64 `store` suite: `run -d 1 blocksro resolve secret`).
- **Asked of the store:** a way for the owner to mint a client of its domain with fewer rights than its own, for instance `domain-client(id, rights)` taking the rights wanted (the get badge's for a read-only start), the client minted with the caller's rights and those together; or the reading client counted as its writing client's owner. The form is STO's.
- **Until then:** the shell refuses `run -d` of a program that only reads, with `RUN -D: NOT YET FOR A PROGRAM THAT ONLY READS: THE STORE MINTS NO READ-ONLY CLIENT OF THE SHELL'S DOMAINS`; a client of the writing one would let it write.
- **Then APP:** the shell mints the read-only domain client for such a start, and the `store` suite's `run_in_domain_check` checks with `blocksro` that it reads the domain's name and is refused a put and a publish.
