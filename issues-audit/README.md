# Code audits and their assessments

This folder keeps the code audits of MIND Core and their assessments. The maintainer audits; an agent assesses each finding against the code.

## Files

- `YYYY-MM-DD-baseline.md`: the state of the repository before an audit (what is done, against the Constitution's stages), with the commits it describes.
- `YYYY-MM-DD-<scope>-audit.md`: an audit, with the commit it was made on.
- `YYYY-MM-DD-validation.txt`: commands, per-suite outcomes and evidence limits for that audit.
- `repro/`: executable audit reproductions; their assertions establish the observed defect, not a successful fix.
- `YYYY-MM-DD-<scope>-assessment.md`: the assessment of that audit. For each finding:
  - its verdict against the code at the audited commit: confirmed, partly confirmed, or not confirmed;
  - its severity and the Constitution clauses it touches (`MC-…`);
  - the track that owns the code;
  - the issue that already covers it, if one exists.

  Then the audit as a whole: what it covers and what it leaves out.

## From a finding to work

A confirmed finding becomes work in its track, as [AGENTS.md](../AGENTS.md) says:

- a task in `issues/` for a track the assessing agent may work in;
- otherwise a request in `issues/requests-<TRK>.md`.

The assessment links each finding to that issue or request. A vulnerability goes the way [SECURITY.md](../SECURITY.md) says, not into this folder.

An audit's results are evidence of what was reviewed at that commit, not a proof (MC-12.2).

## Contents

| File | What |
|---|---|
| [2026-10-09-baseline.md](2026-10-09-baseline.md) | The state of the repository before the first audit |
| [2026-10-09-repository-audit.md](2026-10-09-repository-audit.md) | Code audit at `2cbda21`: eight confirmed findings covering FAT, blockstore, packaging, CI and file-manager/editor data loss; routing and acceptance criteria |
| [2026-10-09-validation.txt](2026-10-09-validation.txt) | Host-test results, skipped checks, existing ELF fixture hashes and audit reproduction output |
| [repro/README.md](repro/README.md) | Commands for ten reproducible checks of the eight findings |
| [2026-10-09-repository-assessment.md](2026-10-09-repository-assessment.md) | Assessment of that audit at `75bbfcf`: all eight findings confirmed against the code; priorities revised for A03, A05 and A07; owners and what the audit leaves out |
