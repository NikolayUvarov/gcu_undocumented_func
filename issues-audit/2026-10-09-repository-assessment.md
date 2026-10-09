# Assessment of the 2026-10-09 repository audit

**Assesses:** [2026-10-09-repository-audit.md](2026-10-09-repository-audit.md) (audited commit `2cbda21`) · **Assessed at:** `fast-test` `75bbfcf` · **By:** the maintainer's assessing session (Claude) · **Date:** 2026-10-09

## What was checked

- **The ten reproductions were run again** at `75bbfcf` with `rustc 1.101.0-nightly (c36f14571 2026-10-01)`. Each printed the same observation as in [the validation file](2026-10-09-validation.txt).
- **The code at every cited location was read**, to check that the mechanism is in the production code and not in the test doubles. The line references checked match.
- **Between `2cbda21` and `75bbfcf`** the files of the findings did not change, except two lines of CI that add the `ehci` suite. They do not touch A06.
- **The tools branch** (`claude/wizardly-franklin-kec1a9`) has since changed `vfs_server/src/fat.rs`, `vfs_server/src/disk.rs` and `fm` (faster FAT reads, 251-APP-0023; the block store as a panel of `fm`). These are not fixes of the findings, but fixes of A02–A04 and A08 will have to merge with them.
- **The cited Constitution clauses** (MC-4.4, 4.5, 4.8, 4.9, 12.2, 12.3, 12.9) were read; they fit the findings they are cited for.
- **Open issues and the `requests-*.md` files** were searched for each finding. None covers one. `vfs_server` has no owner in [TRACKS.md](../TRACKS.md), as the audit says.

**Not run:** QEMU, the full local gate, the external FAT tools, hardware. Like the audit, this is evidence of what was checked at that commit, not a proof (MC-12.2).

## Verdicts

Priority here weighs the impact and how likely the trigger is in the system as it runs today: the block store on a RAM disk, FAT on USB and SATA disks.

| ID | Audit | Verdict | Priority here | Owner | Existing issue |
|---|---|---|---|---|---|
| A01 | P1 | confirmed | P1 | `PRT` (image scripts), build order with `KRN` | none |
| A02 | P1 | confirmed | P1 | `vfs_server`: no owner | none |
| A03 | P1 | confirmed; the trigger is broader than stated | P2 | `vfs_server`: no owner | none |
| A04 | P2 | confirmed; a profile statement is false meanwhile | P2, the profile row now | `vfs_server`: no owner | none |
| A05 | P1 | confirmed | P2 now; P1 before the store gets a durable medium | `STO` | none |
| A06 | P1 | confirmed, scoped exactly | P1 | CI is in no track's directories; the kernel session keeps it | none |
| A07 | P1 | confirmed | P2 | `APP` | none |
| A08 | P1 | confirmed | P1 | `APP` | none |

All eight findings are real defects of the production code. None was refuted.

## Per finding

### A01 — the packager lists programs before it builds

- `APPLICATIONS` is computed at import ([scripts/make_usb_image.py:29](../scripts/make_usb_image.py)). The x86 branch of `files()` reuses it (line 64), and `02_build.sh` runs later, inside `main()` (lines 310–313).
- The maintainer's documented path hits it on a clean clone: `./04_make_usb_image.sh --force`, the steps of [211](../issues/211-intel-pc-from-a-sata-ssd.md). On 2026-10-08 it produced an image with only the boot services, found while building one for the MacBook Pro.
- The fix is small. The audit's acceptance criteria fit: check the image against the build's outputs, not against the packager's own list.

### A02 — a failed FAT write loses the free space

- `write_raw` ([vfs_server/src/fat.rs](../vfs_server/src/fat.rs), from line 463) allocates and links the whole extent before it writes data. `allocate()?` returns on `NoSpace` with the chain linked.
- The new first cluster is only in the local `node`. `write` returns before `store`, and the `Write` handler ([vfs_server/src/main.rs](../vfs_server/src/main.rs), 258–275) returns before `refresh`.
- For an empty file the chain is unreachable. For a non-empty file the new clusters stay linked past its size: reachable, but wasted.
- `check()` only reads, and MIND Core has no repair. The space stays lost until an external `fsck`.
- The trigger is ordinary: any write larger than the free space, such as copying a large file into `data/`. P1.

### A03 — a failed case-only rename deletes the file

- The `case_change` branch ([fat.rs:678](../vfs_server/src/fat.rs)) unlinks the entry before `link` writes the new one.
- `link` needs `parts + 1` *contiguous* free slots (`free_slots`, line 531), so the trigger is broader than "a full root":
  - in the fixed FAT12/16 root, it fails when no such run exists, and fragmentation is enough;
  - in a subdirectory, it grows the directory and fails on a full volume;
  - any I/O error between the two calls has the same effect.
- It loses a whole file, but only on a rename that changes only the case of the name, under those conditions. P2.

### A04 — the volume can read clean after unflushed or failed writes

- **Path 1.** `set_fat_bytes` reads a FAT sector, then `write_sector` calls `changing()`, which writes the dirty bit into the same sector. The stale copy then overwrites it.
  - This happens when the first write after a flush is a FAT update in the first sector of the first FAT copy: clusters below 256 on FAT16, below 128 on FAT32. FAT12 has no clean bit.
  - The second FAT copy keeps the dirty bit, but `check()` reads the first.
- **Path 2.** `flush()` sets the clean bit and clears `changed` before it knows whether `disk.flush()` succeeded. A failed flush always leaves the volume reading clean.
- **Order.** `disk.rs` writes the cache in LBA order, so FAT sector 0 with the clean bit reaches the medium before the data it vouches for.
- **The profile.** [docs/profile/evidence.md](../docs/profile/evidence.md) says "any change marks the volume dirty until a flush". That is not true under path 1. Until the fix, the row should be narrowed (MC-12.3). The code fix is P2.

### A05 — the block store acknowledges a block it erased

- `pass(true)` erases unretained records as it scans ([blockstore/src/store.rs](../blockstore/src/store.rs), line 434), but drops them from the index only after the whole scan (527–533).
- A read failure returns `Device` midway (line 417), leaving erased records indexed. `put`'s fast path (887) then returns the CID without writing.
- That breaks `put`'s own contract ("once the device has flushed it") and MC-4.4.
- Today the store runs on a RAM disk ([docs/storage](../docs/storage/README.md)), where a device read failure is improbable: P2. It is a condition for a durable medium (stage IV), and P1 then.

### A06 — CI can pass a host test that did not compile

- GitHub's default shell for `run:` is `bash -e`, which does not apply to the left side of `&&`. A host test that does not compile is skipped, and the step passes if its last command does.
- A failing test *binary* still fails the step: it is the last command of its list. The audit's wording, compilation failures, is exact.
- Locally, `x86_fixtures` ([scripts/ci_local.sh](../scripts/ci_local.sh)) builds eight kernel variants without `|| return 1` under `set -uo pipefail`. An earlier failure is masked, and stale `/tmp/mind-*-target` builds can be tested.
- The same builds in GitHub CI run as separate commands under `-e`, so they do fail there.
- The gate decides what reaches `main` (AGENTS.md section 4). P1, and a fix of a few characters.

### A07 — saving destroys an existing `<name>.tmp`

- `edit` uses `File::create` (`MODE_TRUNCATE`, [libmind/src/fs.rs:217](../libmind/src/fs.rs)).
- `fm`'s editor calls `create(&temporary, true)`, which [fm/src/main.rs](../fm/src/main.rs) maps to `MODE_TRUNCATE` (lines 82–84).
- `MODE_NEW` exists (fs.rs:18).
- It needs a file named exactly `<name>.tmp` beside the one saved, or two editors saving one file. P2.

### A08 — `fm` removes a move's source before the destination is on its medium

- Removals are planned after the copies ([fm/src/fm.rs](../fm/src/fm.rs), 559–562).
- `finish()` flushes the sources before the target (599).
- `Disk::flush` returns `()` (line 63), and `fm/src/main.rs:94` drops the error of `root.flush()`.
- In a move between two durable volumes (`data/` to a USB stick), the source's removal reaches its medium before the destination's data. A later I/O error or power loss loses the file, and `fm` reports it moved. P1.

## The audit as a whole

**Sound:**
- the reproductions run the production code;
- it says what was simulated, and keeps an environment failure apart from a defect;
- it claims no QEMU or hardware result;
- it routes findings without creating tasks in other sessions' tracks;
- its priorities are defined.

**Corrections:**
- A03's trigger is broader than stated;
- A04's path 1 has a stated condition;
- A05 and A07 are lower in priority here, for how likely their triggers are today.

**Not covered**, as its scope statement says. These are for later audits:
- the kernel's `unsafe` code, IPC and capability paths beyond targeted checks;
- SMP and concurrency;
- drivers' DMA and reset paths: xHCI, the new EHCI, AHCI, NVMe, HDA;
- the bootloader's recent changes for the Mac;
- the network-facing code (`tls`, `keystore`, `netstack`, `netpolicy`, the session parsers);
- `loader` and `init`'s grants;
- the update chain beyond its tests;
- aarch64.

## Routing

**The maintainer decided (2026-10-09): the kernel track assigns the owners.** The request is "Owners for the findings of the 2026-10-09 audit" in [issues/requests-KRN.md](../issues/requests-KRN.md). It proposes:

- **A02–A04:** an owner for `vfs_server`, which has none; `STO` (state and recovery) is the nearest track. The profile row of A04 can be narrowed at once by whoever takes it.
- **A01:** `PRT`. **A06:** `KRN`, which keeps CI, with `ASR` for the gate's tests.
- **A05:** `STO`. **A07** and **A08:** `APP`.

Each fix turns its audit probe's assertion around into a regression test.

**Decided by the kernel track (2026-10-09), main task [175](../issues/175-audit-2026-10-09.md):**
- `vfs_server` joins `KRN`.
- A01: [175-PRT-0007](../issues-done/175-PRT-0007-image-lists-programs-after-the-build.done), done.
- A02: [175-KRN-0047](../issues/175-KRN-0047-fat-failed-growth-gives-clusters-back.md).
- A03: [175-KRN-0048](../issues/175-KRN-0048-fat-case-rename-keeps-the-file.md).
- A04: [175-KRN-0049](../issues/175-KRN-0049-fat-clean-only-after-a-good-flush.md); its profile row is narrowed in the same commit.
- A05: a request in [requests-STO.md](../issues/requests-STO.md).
- A06: [175-KRN-0046](../issues-done/175-KRN-0046-ci-fails-on-every-build-failure.done), done.
- A07, A08: requests in [requests-APP.md](../issues/requests-APP.md), which `APP` may start at once.
