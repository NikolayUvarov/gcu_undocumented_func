# Download recheck stall on the Intel gate host — 2026-10-16

**Finding H13:** observed test failure; underlying cause not established.
**Owner:** NET, with KRN/VFS investigation if the blocked operation is storage.
**Priority:** P1 validation/reliability. **Roadmap:** D / 550, H / 213.
**Constitution:** MC-6.1, MC-12.1, MC-12.3.

## Exact configuration and observation

The x86 gate candidate was ASTRA
`ac1854f202ac8ceebb2167c8b3e0da4a539fdd33`, with main
`bd7b5493d22f5d818c5c07b07ba4873dbaba4b60`, published to fast-test before
tests. The Intel Effector endpoint was `i9_4e6882212251`; operation
`20261015221739.924407850_000152` ran the standard group
`x86: RAM above 4 GiB` and failed after 715 seconds. The command's test
selection was `tests/qemu_smoke.py --memory 6G --suites normal,display,net,vfs`
with the gate's normal firmware/artifact settings. The 125-file artifact
manifest SHA-256 was
`08ee8e3953865138f1db3d7463be3abaa5cffde6c4f29211152fcbb997e45677`.
Source and artifact identity checks remained unchanged.

The memory checks passed: 6014 MiB frame pool, 3071 MiB above 4 GiB,
a 144 MiB heap, and 93 simultaneous clocks with successful reclamation.
The failure happened later in the network suite's `download_runs`, at the
second invocation for an already complete 31,457,280-byte file. The first
download, interruption, resume and expected SHA-256 check had succeeded.

The last program lines were:

```text
DOWNLOAD: RESUMING data/big.bin AT 31457280
DOWNLOAD: ASKED FOR THE REST FROM BYTE 31457280
DOWNLOAD: ITS HEAD PARSED: STATUS 416
DOWNLOAD: ALREADY COMPLETE, 31457280 BYTES
```

The expected SHA-256 line and shell prompt did not arrive within the existing
300-second second-run limit (`tests/qemu_smoke.py::download_runs`, line 5408
at this candidate). Scoped log retrieval was operation
`20261015223334.007515729_000159`; the private local operation record retains
the failure and traceback. This is not a confirmed RAM allocator, NIC,
SHA-256, filesystem or scheduler defect. The case has not been rerun or
reclassified as passing. Independent gate groups can still run.

## Source boundary and requested investigation

At the reviewed candidate, `download/src/main.rs` prints the last line,
calls `output.file.flush()` and discards its result, then calls `sha256`.
That function reads the file in 16 KiB VFS requests before printing the hash.
Neither the flush boundary nor hashing progress is logged. The enclosing
network request timeout has already ended, so this trace cannot distinguish
a blocked flush/read from slow hashing or scheduling. FAT sequential reads
already use a chain cursor; a quadratic traversal must not be assumed.

The [NET request](../issues/requests-NET.md#download-complete-file-recheck-stalls-in-the-6g-intel-gate)
asks for evidence identifying the blocked boundary, an owned task for the
correction, explicit flush-error handling, and regression validation without
weakening the digest check or increasing its timeout to hide the failure.
If VFS/block service work is required, the owner should route that portion
to KRN. Preserve this original failure alongside any later successful run;
physical MacBook/Ryzen network acceptance remains separate.
