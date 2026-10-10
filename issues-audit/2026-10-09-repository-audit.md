# Repository audit — 2026-10-09

**Type:** implementation and assurance audit · **Author:** Codex · **Status:** findings confirmed, fixes not implemented

**Audited commit:** `2cbda21b9a7af31bb12c6e90e92514e2fcf98a7e`, checked out as `fast-test`.
The local `origin/main` reference was `ae8044481105bc571ada57cb05ada7996b624ca2`,
21 commits behind this HEAD. Remote refs and GitHub CI were not refreshed; this
report does not certify current upstream or the merge gate.

The existing [baseline](2026-10-09-baseline.md) and this directory's README were
present before the audit, as untracked files. They are not code-review evidence
by themselves. No production source, branch, issue status or remote was changed.

## Findings

Eight confirmed findings: seven P1 and one P2. P1 here means a reproducible loss
of data/capacity, an unusable delivery, or a check that can accept failed builds;
P2 means incorrect recovery diagnostics. These are remediation priorities,
not vulnerability severity scores. No P0 finding was established.

| ID | Priority | Problem | Routing |
|---|---|---|---|
| [A01](#a01--x86-usb-packaging-omits-applications-built-by-the-same-invocation) | P1 | Fresh x86 USB packaging omits applications | PRT; coordinate build work with KRN |
| [A02](#a02--failed-fat-growth-consumes-unreachable-clusters) | P1 | A failed FAT write consumes all free clusters | VFS owner to be assigned |
| [A03](#a03--a-failed-case-only-rename-deletes-the-original-fat-file) | P1 | A failed case-only rename deletes the original file | VFS owner to be assigned |
| [A04](#a04--fat-can-report-clean-after-unflushed-or-failed-writes) | P2 | FAT reports clean after unflushed/failed writes | VFS owner to be assigned |
| [A05](#a05--an-interrupted-collection-leaves-blockstore-acknowledging-erased-data) | P1 | `blockstore` acknowledges erased data after a failed collection | STO |
| [A06](#a06--ci-shell-control-flow-can-hide-compilation-failures) | P1 | CI and local fixture generation can return success after compiler failures | KRN, with ASR |
| [A07](#a07--saving-a-file-destroys-an-unrelated-existing-tmp-file) | P1 | Editors overwrite an existing neighboring `.tmp` file | APP |
| [A08](#a08--fm-deletes-move-sources-before-confirming-the-destination-flush) | P1 | `fm` deletes move sources before flushing the destination and ignores flush failures | APP |

Routing is advisory, not a claim to a track or a newly numbered implementation
task. `TRACKS.md` does not assign `vfs_server`; the existing
[351-UPD-0008](../issues-done/351-UPD-0008-update-zone-in-vfs.done) explicitly records
that ownership gap. A02–A04 need a maintainer-assigned owner. This audit does not
silently assign VFS to STO or create tasks in another session's track.

## Scope and validation

The review concentrated on implemented paths across build/packaging, CI, FAT and
its VFS callers/cache, block-store mutation and collection, editor/file-manager
saves and moves. Kernel ELF loading, capability/memory operations, x86 context
entry, boot-slot/trial logic, IDL buffer handling and HTTP download code received
targeted inspection and relevant existing host tests, not exhaustive review.

Evidence consists of source inspection plus executable host reproductions:

- **40 Rust host test binaries:** all returned zero, reporting **348 passing
  cases**. Seven FAT cases returned early because `mkfs.fat`, `fsck.fat` and
  mtools are absent; another FAT case omitted its external-tool subcheck.
  Those skipped checks are not passing FAT interoperability evidence.
- **Six Python suites, 49 cases:** passed. `release_test.py` initially failed
  because the sandbox prohibited creating its local HTTPS socket. A permitted
  rerun outside that restriction passed all eight cases; this was an environment
  failure, not a repository defect.
- **Ten audit probes:** reproduced the eight findings (four FAT cases, one
  store case, three packaging/gate probes and two file-manager cases). The
  [reproduction files](repro/README.md) import actual project implementations;
  success means the defect was observed, not fixed.
- The pinned compiler was `rustc 1.101.0-nightly (c36f14571 2026-10-01)`, selected
  by `nightly-2026-10-02`. Existing `usb_root/*.elf` fixtures were used by
  `tests/runtime.rs`; they were not rebuilt or certified as products of HEAD.
  Their hashes and per-suite results are in [validation](2026-10-09-validation.txt).

**Not run:** a complete OS build, QEMU suites, aarch64 execution, real hardware,
TLC, or the full local merge gate. Native QEMU and FAT tools were absent. No
external network retrieval, index rebuild, installation or hardware writes were needed.
The audit does not establish absence of vulnerabilities, architecture-wide
isolation, cryptographic soundness, or full Constitution conformance. MC-12.1,
MC-12.2 and MC-12.9 limit the conclusions to the stated source and configurations.

## A01 — x86 USB packaging omits applications built by the same invocation

**Priority:** P1 · **Verdict:** confirmed · **Routing:** PRT / KRN build
coordination · **Basis:** roadmap track H, MC-12.2, MC-12.9

**Location:** [scripts/make_usb_image.py](../scripts/make_usb_image.py), lines
29, 59–65 and 310–313; [.github/workflows/ci.yml](../.github/workflows/ci.yml),
the USB image step.

`APPLICATIONS` is captured from `usb_root/*.elf` at module import, before
`main()` runs `02_build.sh`. The x86 branch of `files()` reuses that tuple after
the build. On a clean checkout the tuple is empty; new application binaries in
an incremental build are also absent. Boot services are selected separately,
so a bootable image can still omit the user programs. The aarch64 branch
enumerates applications later and is not affected by this particular timing bug.

The built-in image check compares against the already incomplete `payloads`.
CI builds first and invokes the packager with `--no-build`, avoiding the failing
first-build order. Thus neither check detects the omission.

**Reproduction:** `python3 issues-audit/repro/build_repro.py` executes the actual
module in an empty temporary repository, creates `usb_root/audit-app.elf` after
import, and confirms that `files('x86_64')` omits it. This verifies enumeration;
it does not claim a QEMU boot or image conversion was performed.

**Plan / acceptance:** enumerate applications from the selected source after
the build. Test one-call packaging from a clean tree and after adding an app,
checking image contents against independent build outputs rather than the
packager's own precomputed list.

**Related:** already noted, but not experimentally assessed, in baseline item 2;
[011](../issues-done/011-reproducible-toolchain.done).

## A02 — failed FAT growth consumes unreachable clusters

**Priority:** P1 · **Verdict:** confirmed · **Routing:** unassigned VFS
maintenance · **Basis:** MC-4.8, MC-12.2

**Location:** [vfs_server/src/fat.rs](../vfs_server/src/fat.rs), lines 463–476,
496–506 and 510–514; [vfs_server/src/main.rs](../vfs_server/src/main.rs), lines
258–275.

`write_raw()` allocates and links clusters for the entire requested extent
before writing data. If allocation reaches `NoSpace`, `?` returns without
releasing those clusters. The node's first cluster can already have changed,
but `store(node)` has not updated the directory entry. The VFS handler uses a
local node copy and also returns on error before `refresh()`, losing that copy.

A normal oversized write to an empty file can therefore consume the volume's
remaining capacity as lost clusters. Removing that empty file cannot free them:
its directory entry still has cluster zero. The growing `truncate()` path uses
the same allocation helper.

**Reproduction:** `failed_growth_leaks_all_free_clusters` in
[fat_repro.rs](repro/fat_repro.rs) formats a 128-sector memory disk and requests
one cluster more than the available 93. Observed: `NoSpace`, free count `93 → 0`,
93 lost clusters, directory cluster zero; deletion restores no capacity.

**Plan / acceptance:** make failed growth preserve reachable allocations or
roll back newly allocated chains. Exercise empty and nonempty files, writes
with gaps and growing truncation under `NoSpace` and injected I/O failures;
check free counts, file contents and a remounted filesystem. Existing data
must remain reachable, with no lost or cross-linked clusters.

**Related:** [066](../issues-done/066-vfs-v2-fat-write.done);
[profile evidence](../docs/profile/evidence.md), the FAT consistency statement.

## A03 — a failed case-only rename deletes the original FAT file

**Priority:** P1 · **Verdict:** confirmed · **Routing:** unassigned VFS
maintenance · **Basis:** MC-4.8, MC-12.2

**Location:** [vfs_server/src/fat.rs](../vfs_server/src/fat.rs), lines 567–574
and 654–679, especially the `case_change` branch at 678.

For a name change that resolves to the same entry, `rename()` unlinks the old
entry before attempting to create the replacement. Changing an all-lowercase
8.3 name to mixed case can require an additional long-name slot. When the fixed
FAT12/16 root is full, the newly freed short-entry slot is insufficient and
`link()` returns `NoSpace`. The original name has already disappeared, leaving
its data clusters unreachable.

**Reproduction:** `failed_case_change_deletes_the_original_file` creates
`alpha.txt` with contents, fills the root with 510 other files (plus the volume
label), and renames it to `AlPhA.txt`. The call returns `NoSpace`, lookup of the
original returns `NotFound`, and the checker reports one lost cluster. No
power loss, malformed volume or device error is needed.

**Plan / acceptance:** reserve sufficient slots or arrange a recoverable update
before removing the old name. Test mixed-case expansion in full and fragmented
directories and failed sector writes. On refusal, the original file and its
contents must remain accessible and filesystem consistency must be preserved.

**Related:** [066](../issues-done/066-vfs-v2-fat-write.done);
[tests/fat_host.rs](../tests/fat_host.rs), the existing successful case-change test.

## A04 — FAT can report clean after unflushed or failed writes

**Priority:** P2 · **Verdict:** confirmed · **Routing:** unassigned VFS
maintenance · **Basis:** MC-4.8, MC-12.2, MC-12.3

**Location:** [vfs_server/src/fat.rs](../vfs_server/src/fat.rs), lines 227–231,
242–251, 279–304 and 775; [vfs_server/src/disk.rs](../vfs_server/src/disk.rs),
the LBA-ordered `flush()`.

Two paths undermine the documented dirty indication:

1. `set_fat_bytes()` reads a FAT sector before `write_sector()` calls
   `changing()`. On the first allocation after a flush, that saved sector can
   still contain the clean bit. Writing it back overwrites the dirty bit that
   `changing()` just set in the first FAT copy. The in-memory `changed` flag
   masks this in `check()` until the volume is mounted again.
2. `flush()` sets the clean bit and clears `changed` **before** it knows whether
   `disk.flush()` succeeded. A failed flush returns `Io` while a subsequent
   check reports clean. In production the dirty and clean transitions also
   pass through a write-back cache; LBA ordering is not a durability barrier.

**Reproduction:** the last two checks in [fat_repro.rs](repro/fat_repro.rs)
observe `FAT[1] & 0x8000 == 0x8000` and `dirty=false` after an unflushed
allocation/remount, and `dirty=false` after an explicitly failed flush.
These are memory-medium tests, not a measured physical power-loss result.

**Plan / acceptance:** establish the dirty state before mutations can reach
the medium, avoid overwriting it from a stale sector buffer, and mark clean
only after successful data/metadata durability barriers. Preserve a dirty or
indeterminate state after errors. Inject faults at each barrier and remount;
an incomplete write must not be reported as a successful clean shutdown.

**Related:** [profile evidence](../docs/profile/evidence.md), the explicit
claim that changes mark the volume dirty until a flush; this claim is narrower
than, and does not require promising, atomic FAT transactions.

## A05 — an interrupted collection leaves blockstore acknowledging erased data

**Priority:** P1 · **Verdict:** confirmed · **Routing:** STO · **Basis:**
MC-4.4, MC-4.5, MC-4.8

**Location:** [blockstore/src/store.rs](../blockstore/src/store.rs), lines
409–438, 527–533 and 883–887; [blockstore/src/main.rs](../blockstore/src/main.rs),
the `Collect` and `Put` request handlers.

The sweep erases unretained records as it scans, but removes their index entries
only after the entire scan succeeds. A read/write/flush failure after an erase
exits with the erased record still indexed. The service replies with the error
and continues serving the same store. A later `put()` of the erased content
hits the deduplication fast path, renews the stale lease and returns its CID
without writing or verifying a record.

**Reproduction:** [blockstore_repro.rs](repro/blockstore_repro.rs) stores two
blocks, expires their leases and injects one failed read after the first block
has been erased. `collect()` returns `Device`; `has()` still returns true;
`put()` of the same bytes returns `Ok(cid)`; the immediate `get()` returns
`Corrupt`. The original expired block was eligible for collection; the defect
is the false acknowledgment of the subsequent put, not its initial removal.

**Plan / acceptance:** keep index/free-space state consistent with every
completed erase, or stop using the store until a successful rescan after an
indeterminate failure. Inject failures throughout sweep/erase and then retry
puts on the same service instance. Every successful put must be readable under
the medium's declared durability contract.

**Related:** [303-STO-0001](../issues-done/303-STO-0001-collection-by-reachability.done),
[storage contract](../docs/storage/README.md). The host reproduction does not
extend a RAM-backed deployment's lifetime beyond reset.

## A06 — CI shell control flow can hide compilation failures

**Priority:** P1 · **Verdict:** confirmed · **Routing:** KRN, with ASR ·
**Basis:** MC-12.2, MC-12.9; AGENTS.md section 4

**Location:** [.github/workflows/ci.yml](../.github/workflows/ci.yml), lines
30–38; [scripts/ci_local.sh](../scripts/ci_local.sh), lines 110–124.

The GitHub host-test step uses `rustc ... && test-binary` without an explicit
failure return. Bash's `-e` does not exit when the left side of that `&&` fails;
later suites and Python commands can succeed, yielding a successful step
although a Rust suite never compiled. This also applies to loop iterations.
The local `host_tests()` function already handles this correctly with
`|| return 1`; the GitHub step does not.

Separately, local `x86_fixtures()` runs eight kernel feature builds without
checking their results. The script has `set -uo pipefail`, not `set -e`, and
the function returns the final build's result. An earlier feature-build failure
can be reported as successful fixture preparation. On a reused host, stale
`/tmp/mind-*-target` artifacts can then be tested instead of a freshly built
fixture. The reproduced claim is the false function status, not that a full
QEMU gate was run and accepted.

**Reproduction:** [build_repro.py](repro/build_repro.py) extracts and executes
the actual shell bodies with isolated no-op build commands. Injected exit 42
from the runtime-test compiler yields host-step exit 0; injected exit 42 from
the `panic-test` build yields `x86_fixtures` exit 0.

**Plan / acceptance:** propagate every compiler/build failure explicitly and
reject stale fixtures. Fault each build position in the CI host step and local
fixture function; each must return nonzero and prevent a PASS gate. Check test
execution failures as well as compilation failures.

**Related:** [011](../issues-done/011-reproducible-toolchain.done),
[local gate rules](../AGENTS.md).

## A07 — saving a file destroys an unrelated existing `.tmp` file

**Priority:** P1 · **Verdict:** confirmed · **Routing:** APP · **Basis:**
roadmap track G; MC-4.8

**Location:** [edit/src/main.rs](../edit/src/main.rs), lines 51–64;
[fm/src/fm.rs](../fm/src/fm.rs), lines 434–447;
[libmind/src/fs.rs](../libmind/src/fs.rs), line 217;
[fm/src/main.rs](../fm/src/main.rs), lines 82–84.

Both editors use the fixed staging name `path + ".tmp"` and create it with
truncation/replacement enabled. An existing file with that name is overwritten
without checking ownership or asking about that file. On successful save it
is renamed away; on failure cleanup may delete it. A normal F2 save can thus
destroy an independent neighboring file or a recovery file from another save.

**Reproduction:** the save case in [fm_repro.py](repro/fm_repro.py) creates
`todo.txt` and a separate `todo.txt.tmp`, opens the former in the actual built-in
editor, types text and presses F2. The target is saved and the unrelated `.tmp`
file disappears. The standalone editor's equivalent behavior is established
by inspection of its `File::create` call and `MODE_TRUNCATE` implementation;
its GUI was not run.

**Plan / acceptance:** create a staging file exclusively using `MODE_NEW` or
an equivalent no-clobber operation; select another name on collision. Track
which temporary file this save actually owns. Verify existing staging names,
concurrent saves and failure cleanup without altering another file's contents.

**Related:** [067](../issues-done/067-editor.done),
[068](../issues-done/068-fm-write-df-fsck.done). FAT's documented best-effort
replacement does not explain overwriting an unrelated temporary-name collision.

## A08 — fm deletes move sources before confirming the destination flush

**Priority:** P1 · **Verdict:** confirmed · **Routing:** APP · **Basis:**
MC-4.8, MC-4.9; roadmap track G

**Location:** [fm/src/fm.rs](../fm/src/fm.rs), lines 60–63, 239–272, 559–562
and 595–603; [fm/src/main.rs](../fm/src/main.rs), line 94.

A cross-volume move schedules copies followed by source removals. Destination
writes are only acknowledged into VFS's write-back cache. `finish()` flushes
volumes after the removal steps have completed, visiting sources before the
target. There is no successful destination-flush checkpoint before deleting
the original. Moreover, `Disk::flush` returns `()`, and the real implementation
discards `root.flush()` errors, so `finish()` can report `Moved` despite a
failed destination write/flush.

For example, moving a RAM file to `data/` can remove the RAM copy before the
boot disk reports an I/O failure. The application no longer retains the
source needed to retry. The void flush contract also prevents the built-in
editor from reporting its final flush failure.

**Reproduction:** the move case in [fm_repro.py](repro/fm_repro.py) observes the
actual job engine deleting the source while the disk double's flush counter
is zero. The test demonstrates ordering; error suppression is directly visible
in `fm/src/main.rs`. It does not simulate a physical device losing power, and
does not claim that a RAM destination should survive reset.

**Plan / acceptance:** make flushing fallible and require a successful
destination flush before deleting the corresponding source. Keep the source
and show a failure when persistence fails. Test destination writeback/flush
errors, multi-file moves, retry and cancellation; source deletion must not
precede the required success checkpoint. Save/copy completion must propagate
flush errors rather than reporting success.

**Related:** [068](../issues-done/068-fm-write-df-fsck.done),
[vfs_server/src/disk.rs](../vfs_server/src/disk.rs), the write-back contract.

## Existing gaps and follow-up

The stale stage I–III descriptions in [ROADMAP.md](../ROADMAP.md), section 2,
remain inconsistent with its own completed K4/C1–C8 items. This was already
recorded in the baseline; it is not counted again among the eight implementation
findings. Any repair must follow the repository's versioning and bilingual rules.

Open work such as IOMMU isolation, the unfinished updater, platform-specific
hardware validation and per-session parser processes was not relabeled as a
new defect merely because it is incomplete. These remain governed by their
existing issues and declared profile limits.

Prioritize A02/A03/A07 for direct user-data loss, A06 before relying on the gate,
then A05/A08 and A01. Fixes need regression tests with assertions inverted from
the audit probes, plus the suites and profile updates required by their owning
tracks. This report is evidence of the stated review and failures, not a proof
of the rest of the system (MC-12.2).
