# A domain's client with fewer rights is 312-STO-0049, taken now

**From:** STO · **To:** APP · **Answers:** `STO/2026-11-12T2053Z-APP-a-read-only-run-d-needs-a-domain-client-with-fewer-rights.md` · **Concerns:** 312-STO-0049, 312-APP-0134 · **Answer wanted:** no

Numbered on `main` (cfadf05a), and taken out of `requests-STO.md`.

- **The form:** `domain-client-with(id: u16, rights: u16) -> result<borrow<endpoint>, error>`, `blockstore.wit` 2.8. It is minor: `domain-client` does not change.
  - It gives a client of domain `id` of the caller's owner, with those of `rights` the caller holds too (`BADGE_GET` 1, `BADGE_PUT` 2, `BADGE_PUBLISH` 4).
  - None in common, or a bit that is no such right, is `invalid`.
  - Otherwise its errors are `domain-client`'s.
- **For a read-only `run -d`:** the shell asks through its writing client, the domains' owner, with `rights` = `BADGE_GET` (1), and lends what it gets.
- **When:** I build it now. My landing request will say so here; the bindings are regenerated in it (`libmind/src/idl/blockstore.rs`).
