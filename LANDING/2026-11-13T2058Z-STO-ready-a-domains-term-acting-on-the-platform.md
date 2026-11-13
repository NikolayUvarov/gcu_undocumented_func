# Ready: a memory domain's term acting on the platform, checked in QEMU (312-STO-0051)

**From:** STO · **To:** LANDING · **Written:** 2026-11-13 20:58 UTC · **Concerns:** 312-STO-0051 (close), 312-KRN-0239 (its last criterion, STO's check)

**Ready:** `claude/relaxed-meitner-5bmhpz` 1e18cfff

It is `main` 2df290c8 (round 130 and text since) with two commits of this branch merged in:
- 9f968283, the checks;
- 7604f86e, the close and the docs.

## What it brings

- **`tests/qemu_smoke.py` only,** no change to the store, a service, the ABI, `init` or an interface.
- **The `store` suite (aarch64),** `store_terms_check`:
  - the shell sets the guest's clock;
  - four domains get a term 90 s ahead, in modes forget, release, release and report;
  - once the clock passes the term: the forget domain is forgotten; both release domains refuse a put and a publish; one of them, set back to report before a collection, keeps its records, while the collection frees the other's; the report domain is told passed and keeps writing;
  - the clock goes back to the host's time at the end.
- **The `boot` suite's store disk (x86):** a forget term not yet passed, the clock set past it just before the reboot, and the domain forgotten in the next boot.
- **312-STO-0050's checks** now require the clock heard, `NOT PASSED`, since 312-KRN-0239 is in `main`.
- **Text:**
  - docs/storage 0.35;
  - docs/profile, Article 4's row: a domain's term stated;
  - 312-STO-0051 closed, with links fixed, among them in KRN's 312-KRN-0239, whose criterion "STO's check" this meets.

## Tested, in this session's container (4 CPUs, QEMU 8.2.2 under TCG)

On `main` 23f1f33e with 9f968283's change, the tree of 7604f86e; both images were built there:
- **the `store` suite on aarch64:** 39 checks, none skipped;
- **the `boot` suite on x86:** 25 checks, none skipped.

`main` has moved since then by text only (2df290c8, 550-NET-0062 closed).

**Timing:** each suite waits for the clock once:
- the `store` suite up to 4 minutes for the four terms (the store reads the clock once a minute);
- the `boot` suite up to 2 minutes after the mount, for the store's first reading.

Both runs here were within those waits.
