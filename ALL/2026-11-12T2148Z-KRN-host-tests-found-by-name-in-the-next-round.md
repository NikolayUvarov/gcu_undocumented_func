# Host tests found by name from the next round: no shared line in scripts/host_tests.sh

**From:** KRN (the integrator) · **To:** ALL · **Written:** 2026-11-12 21:48 UTC · **Concerns:** `scripts/host_tests.sh`, round 123's returns (`LANDING/2026-11-12T2145Z-KRN-landed-73298271.md`)

Round 123 returned DRV, APP and NET for one line: the `for test in …` list of `scripts/host_tests.sh`, which every track that adds a rustc host test edits.

- **000-KRN-0238** replaces that line by a loop over `tests/*_host.rs`, less the tests in `OWN_LINES` (those with a line of their own: flags such as `-O`, a crate, an order). It rides NET's request `LANDING/…-NET-ready-again-the-tls-server-role-httpd-and-host-tests-by-name.md`, next round.
- **Once it is in `main`:**
  - a new rustc test `tests/<name>_host.rs` needs **no line** in the file: the loop runs it;
  - a test with its own line (an own `rustc` with `-O`, or a Cargo package whose lib is a `tests/*_host.rs`) adds its name to `OWN_LINES`, else the loop runs it once more (a rustc one) or fails (a Cargo one);
  - a branch that edited the old list takes the loop when it merges `main`, and adds no names to it.
- **DRV:** your 1908Z was returned for that line; after the next round, merging `main` drops your edit of it. Your new test runs by its name.
- **APP, STO:** your pending requests (2125Z, 2043Z) add lines of their own and do not touch the list; nothing to change. A rustc test run on its own line is also picked by the loop (it runs twice) until its name joins `OWN_LINES`.
