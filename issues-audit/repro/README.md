# Reproductions for the 2026-10-09 repository audit

These checks **assert that the reported defect is present** at commit
`2cbda21b9a7af31bb12c6e90e92514e2fcf98a7e`. A passing check confirms the finding;
it is not a regression test that certifies a fix. Turn the assertions into the
desired behavior when the owning track implements a fix.

Run from the repository root with the pinned Rust toolchain and Python 3:

```bash
rustc --edition=2021 --test issues-audit/repro/fat_repro.rs -o /tmp/mind-audit-fat
/tmp/mind-audit-fat --nocapture
rustc --edition=2021 --test issues-audit/repro/blockstore_repro.rs -o /tmp/mind-audit-store
/tmp/mind-audit-store --nocapture
python3 issues-audit/repro/build_repro.py
python3 issues-audit/repro/fm_repro.py
```

| File | Findings | What is real / simulated |
|---|---|---|
| [fat_repro.rs](fat_repro.rs) | A02–A04 | Imports the real FAT implementation; sector storage and a failed flush are simulated in memory |
| [blockstore_repro.rs](blockstore_repro.rs) | A05 | Imports the real store, CID, SHA-256 and DAG implementations; injects one failed read after an erase |
| [build_repro.py](build_repro.py) | A01, A06 | Executes the packager's actual source in an empty temporary tree; extracts actual CI shell bodies and substitutes compilers/test commands |
| [fm_repro.py](fm_repro.py) | A07–A08 | Extends the existing host test harness in a temporary file; imports real `fm` and editor code and observes deletion in the existing memory disk |

The checks touch temporary files and memory only. They do not build an OS image,
write a physical disk, change production sources, or run QEMU. The file-manager
test checks call ordering, not real-medium power-loss behavior. Its temporary
copy of the existing test harness adds only an observation to `remove`; it does
not change the result of any disk operation. Only the two `audit_` tests run.

See [the audit](../2026-10-09-repository-audit.md) for impact, scope, priorities
and acceptance criteria, and [validation](../2026-10-09-validation.txt) for the
recorded results.
