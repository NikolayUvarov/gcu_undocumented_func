# Assurance: models and fuzzing

**Version:** 1.3 (2026-11-22, [158-ASR-0016](../../issues-done/158-ASR-0016-the-camera-interfaces-and-the-jpeg-decoder-fuzzed.done): the camera interfaces and the JPEG decoder); 1.2 (2026-10-25, [000-KRN-0104](../../issues/000-KRN-0104-revocation-bounded-by-the-profile.md): revocation in steps); 1.1 (2026-10-09, [500-ASR-0001](../../issues/500-ASR-0001-idl-decoder-fuzzing.md): fuzzing the IDL decoders); 1.0 (2026-10-06, issue [167](../../issues-done/167-models-of-revoke-and-move.done)) · **Constitution:** [v1.6](../../constitution/EN/MIND_CORE_Constitution_v1.6.md) MC-2.6, MC-3.4–3.6, MC-12.1, MC-12.2 · **Roadmap:** stage II exit, "Assurance models for revoke/MOVE"

Models of kernel mechanisms in TLA+, checked exhaustively by TLC within stated bounds. They are **models, not proofs of the implementation** (MC-12.2): each abstracts the code it names, and a result holds for the model within its bounds. What they add is a search of every interleaving the bounds allow, which tests do not give. Run them with `scripts/model_check.sh` (it fetches TLC 1.8.0 once and checks its SHA-256; Java 11+); CI runs them in the job "models".

## CapRevokeMove — the capability tree, revoke and MOVE

[`CapRevokeMove.tla`](CapRevokeMove.tla) models `kernel/src/scheduler.rs`: capabilities in task slots with their nodes (`Node { id, parent }`), `CAP_MINT` (`mint`: a child with narrowed rights; a child of a capability without grant is never writable), a copy or a move in flight (`transfer`: one send per task, the sender blocked meanwhile), delivery (`place`: a move empties the sender's slot if it still holds the node, otherwise the send is void; an overwritten slot is removed like a drop), `CAP_DROP` and task exit (`remove`: a ghost node keeps the descendants of a removed capability revocable), `MEM_MAP` and unmapping (mappings remember the node they were made from), and `CAP_REVOKE` in steps (000-KRN-0104, `kernel/src/scheduler/tree.rs`): the root is marked first (`BeginRevoke`); each step removes one slot, copy in flight or ghost whose parent is the root or a node the walk removed, with the mappings made from it (`RevokeStep`), and every other task's action may come between two steps; while the mark stands, a mint, copy or move of anything under the root is refused (`Covered`, the kernel's `ERR_REVOKING`); the last step clears the mark and the call returns (`EndRevoke`). A move in flight goes with its sender's slot, and its delivery is then void.

| Invariant | Statement | Constitution |
|---|---|---|
| `OneWriter` | a move-only object (writable, without grant) is written by one task at most: a writable capability or a writable mapping | MC-2.6 (MOVE: no two owners) |
| `NoRevival` | a node removed by revoke is in no slot, send or mapping again | MC-3.6 |
| `Narrowing` | a capability whose parent is live has no right its parent lacks | MC-3.5 |
| `NoGrowth` | while a revocation goes on, what is under its root stays within what was there as it began: the walk ends | MC-5.4, MC-3.5 (the fate of operations in progress) |
| `Completion` | a child of a node a revocation removed lives only while that revocation goes on | MC-3.6 (a completion point) |

Bounds ([`CapRevokeMove.cfg`](CapRevokeMove.cfg)): three tasks with two slots each, a move-only object and a shared one, node ids up to 4, masks "all rights", "without grant", "read only", one task revoking in steps while the others run. Result: no error; 12,167,313 distinct states, depth 26 (14 minutes on two workers, 2026-10-25; before 000-KRN-0104, with revoke in one step by any task, 6,263,047 states, depth 18). [`CapRevokeMove_revokers.cfg`](CapRevokeMove_revokers.cfg): two tasks, both revoking in steps at once: no error; 1,644,334 distinct states, depth 24.

**What TLC found.** Before issue 167 the kernel moved a memory capability its sender had mapped: `transfer` and `place` checked the sender's slot, not its mappings. [`CapRevokeMove_before167.cfg`](CapRevokeMove_before167.cfg) checks that kernel; TLC's shortest counterexample has three steps: task a holds the move-only object and maps it writable, sends it with `CAP_TRANSFER_MOVE` to b, b receives it — a still writes through its mapping while b holds the writable capability. The `isolation` suite reproduced it (case `z`, before the fix: `MOVE OK` never came). Since issue 167 a memory capability whose node the sender has mapped does not move (the reply or send carries no capability); unmapping first (`FREE` of the mapping) lets it move. `scripts/model_check.sh` checks that the old configuration still fails, so the model keeps the power to find it.

A second counterexample was the model's, not the kernel's: a sender that maps the object after its send is queued. In the kernel a task with a send in flight is blocked in it; the model now says so (`Running`).

**Revocation in steps (000-KRN-0104).** [`CapRevokeMove_unrefused.cfg`](CapRevokeMove_unrefused.cfg) lets a mint, copy or move of a node under a revocation in progress through (`RefuseCovered = FALSE`): TLC finds `NoGrowth` violated, a task minting under the root between two steps of the walk, so the refusal is what makes the walk end. `scripts/model_check.sh` checks that it still fails.

Abstractions: rights are the set {read, write, grant}; endpoints, ports and DMA regions are represented by the shared object (their move rules are the same tree); memory ranges and sub-ranges are not modelled (a mint keeps the whole object); generations of slot handles are left out (a slot is named directly; stale handles are tested by the `isolation` suite); ghosts are unbounded (the kernel's are entries of its index of the tree, 000-KRN-0104); the walk's order and its steps of 256 nodes are not modelled, only that other actions come between any two of its nodes.

## RevokeFlush — the completion point of revoke on several CPUs

[`RevokeFlush.tla`](RevokeFlush.tla) models `revoke` and `select`: a task on each other CPU maps the memory being revoked and caches the translation in its CPU's TLB while it runs; revoke clears the page table entries, marks each CPU that runs an affected task (`flush`), wakes it and blocks the revoker (`BlockedFlush`); a CPU's next `select` reloads its translation root, clears its mark, and the last one releases the revoker. On aarch64 the clear is also broadcast to every TLB (`tlbi vmalle1is`).

| Property | Statement | Constitution |
|---|---|---|
| `Complete` (invariant) | once revoke returns, no page table and no TLB holds the translation | MC-3.6 (a completion point) |
| `Returns` (liveness, with weak fairness of every CPU's switching) | a waiting revoker returns | MC-3.6 |

Bounds: three CPUs, the revoker on one. Result: no error for x86 ([`RevokeFlush.cfg`](RevokeFlush.cfg), 25 states) and aarch64 ([`RevokeFlush_aarch64.cfg`](RevokeFlush_aarch64.cfg), 18 states). A mutation that returns from revoke without waiting is caught (`Complete` violated). Abstractions: one revoke; a holder pinned to its CPU (the kernel pins tasks); holders on the revoker's own CPU are not running and their translations went at their last switch (no PCIDs or ASIDs).

## Fuzzing the IDL decoders (500-ASR-0001)

[`tests/idl_fuzz_host.rs`](../../tests/idl_fuzz_host.rs) is a host test with a fixed seed (`0x5000A5A00001`) and 50 000 inputs per target. It runs the generated decoders of `libmind/src/idl/` unchanged:

- **Every interface's receiver `decode`** (35 interfaces on 2026-11-22) gets messages built from the shape of its generated source (its methods, buffer sizes and reply room), random words, any capability kind, calls and non-calls, and buffer requests mutated from those it accepted. Every message must:
  - not panic;
  - release a received capability, whether the message is accepted or refused (MC-2.12);
  - decode the same way twice.
- **An accepted message must also:**
  - be a call (not an interrupt);
  - be refused as `Version` under another major version.
- **An accepted buffer request must also:**
  - be refused with one trailing byte or with its last byte cut (MC-2.4);
  - keep its decoded value when the client rewrites its whole buffer after decoding, since the receiver decodes a private copy (MC-2.11).
- **Every generated `Wire` type** (211 types on 2026-11-22) decodes random and mutated bytes. Whatever it accepts must not panic and must encode back, within `MAX`, to exactly the bytes it consumed: a decoder accepts only canonical encodings of valid values.
- **The test lists every interface and type,** and fails when the generator adds one the list lacks.

**Results.** No finding at the fixed seed, nor at five other seeds with 200 000 inputs per target, about 100 million decoder runs in all (2026-10-09). Three injected defects were each caught:

- `bool` accepting 2;
- `header` keeping a capability it refused;
- a `vfs` request whose decoder skips the check that the whole request was consumed.

**The camera interfaces** (2026-11-22, LEG9, the fixed seed; 158-ASR-0016): no finding among the 35 receivers and 211 types, and among them:
- `video.wit` 1.2: its receiver's 7 methods (14 792 of 50 000 messages accepted), and its types `Error`, `Camera`, `Frame` and 1.1's `Encoding`, `Mode` and `Opened`. 1.2 adds nothing on the wire;
- `usb.wit` 1.6: its receiver's 11 methods with 1.6's `bulk-stream` (15 407 of 50 000 accepted). The interface defines no `Wire` type.

This is evidence of the inputs that were run, not a proof that no input breaks a decoder (MC-12.2).

**Not covered:**

- the clients' side, the decoding of replies, beyond the `Wire` types it shares;
- whether a decoder accepts exactly what its `.wit` schema allows, because the test has no independent reading of the schema. Canonical round trips and the bounds of `Text` and `List` stand in for it;
- the system calls (500-ASR-0002).

`MIND_FUZZ_SEED` and `MIND_FUZZ_ITERATIONS` change the seed and the count for a longer search.

## Fuzzing the JPEG decoder (158-DRV-0084)

[`tests/image_host.rs`](../../tests/image_host.rs) `mutated_frames_never_panic` is a host test with a fixed seed (`1580084`) and 3000 inputs. It runs `libmind/src/jpegdec.rs` unchanged: the decoder of `wm`'s backgrounds and of a camera's Motion JPEG frames, whose bytes are the device's.

- **The inputs:** seven pictures, with and without their Huffman tables, each mutated by flipped bits, changed, inserted and removed bytes, and cuts:
  - `jpeg.rs`'s 4:2:0 frames;
  - a hand-built 4:2:2 frame without tables, as UVC cameras send;
  - `restart.jpg`, `444.jpg`, `grey.jpg` and `plain.jpg`.
- **What every input must do:**
  - not panic (the host test runs with overflow checks);
  - hand its rows over in order;
  - when accepted, stay within its size limit with as many rows as its height.

**Results:**
- No finding at the fixed seed (2026-11-21 and 2026-11-22, LEG9).
- No finding at 200 000 inputs at that seed, nor at five other seeds with 100 000 inputs each (2026-11-21, LEG9).
- The decoder before 158-DRV-0084 failed it: `attempt to subtract with overflow`, a DC size past the 32 bits held.

This is evidence of the inputs that were run, not a proof (MC-12.2). `MIND_FUZZ_SEED` and `MIND_FUZZ_ITERATIONS` change the seed and the count.

**Not covered:** progressive and 12-bit pictures (refused before decoding), and frames from a real camera: the Ryzen's are to be recorded (158-DRV-0084, criterion 1).

## Threat models

- **[The remote update's threats](remote-update-threats.md)** (751-ASR-0013, 0.1, 2026-10-29): a developer's release installed through Effector under the owner's `remote-update`.
  - Each attack is listed with the check asked of its track and the test asked for it.
  - The test keys' risk: on the development disks the path installs anyone's code.
  - It is a design until 751's parts exist.

## Not modelled yet

Endpoint queues and cancellation (MC-2.7), the reply capability's one use, quotas, the supervisor's restart path, checkpoints and fencing (Appendix D, later stages).
