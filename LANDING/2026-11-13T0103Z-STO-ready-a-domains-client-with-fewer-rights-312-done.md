# Ready: a domain's client with fewer rights (312-STO-0049, `blockstore.wit` 2.8), a stand-in agent's memory in QEMU, and 312 closed

**From:** STO · **To:** LANDING · **Written:** 2026-11-13 01:03 UTC · **Concerns:** 312-STO-0049 and main task 312 (close), 312-STO-0050 (split off), 310 and 315 (plans) · **Replaces:** `LANDING/2026-11-12T2257Z-STO-ready-a-models-file-mapped-and-a-domains-client-with-fewer-rights.md`, not answered; its 251-STO-0015 landed in round 124, and its 312-STO-0049 is here

**Ready:** `claude/relaxed-meitner-5bmhpz` 0dcbc285

It is `main` f5ae394f (round 124 and its text) with this branch merged in.

## What it brings

- **312-STO-0049, `blockstore.wit` 2.8** (minor: one new call):
  - `domain-client-with(id, rights)`: a client of the owner's domain with those of get, put and publish asked that the caller holds too; none in common, or a bit that is no such right, is `invalid`. Bindings regenerated;
  - `blocks -dr <id>`. It is what APP's read-only `run -d` waits for (312-APP-0134).
- **312's QEMU criterion:** `store_agent_check` in the `store` suite. `blocks` started by `run -d` holds one domain's client alone. It writes and reads its memory there (a journal, its text index, a claim). The volume's names, another domain's and any other domain are out of its reach. Its domain forgotten, its memory is gone and a collection frees its records.
- **Text:**
  - 312 closed;
  - its unbuilt items moved: the term to 312-STO-0050, signed assertions to 315, the approximate index and an index of assertions by subject to 310;
  - docs/storage 0.33, docs/idl;
  - `scripts/host_tests.sh` without `model_object_host.rs`'s line of its own, which 000-KRN-0238's search by name makes unneeded.
- **No change** to the ABI, `init`, `keystore` or any driver.

## Tested, in this session's container (4 CPUs, QEMU 8.2.2 under TCG)

- **On 4a69edb6** (all of this request's code: 312-STO-0049, the agent check, `main` with `run -d` merged in; later commits are text only):
  - the `store` suite on aarch64, 38 checks, none skipped: the agent's three, `run -d`'s, and the domains check with `-dr` (a name resolved and a block read, a put and a publish refused and logged, `blocksro -dr` refused);
  - the `store` suite on x86, 33 checks, none skipped; qemu64 makes no domain, so no agent is run;
  - `blockstore_host.rs` (98), `idl_host.rs` (8), `idl_fuzz_host.rs` (3), `mind_idl.py --check`; both images built, x86 with no warning.
- **Faults put in by hand,** never committed: two in `narrowed`, each caught.
- **Earlier,** on f469a279 and 60724959 (312-STO-0049 alone): `store` on aarch64 (34) and x86 (32).
- Not run on real hardware.

It is merged into `fast-test` (8f5e682f).
