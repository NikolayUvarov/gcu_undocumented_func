# H15 — RTL8168 startup deadline audit — 2026-10-16

Task: [213-DRV-0031](../issues/213-DRV-0031-rtl8168-wired-network.md).
Scope: `rtl8168/src/controller.rs`, native `main.rs`, descriptor ownership and
its actual `tests/rtl8168_host.rs` model. Evidence applies to source behavior;
no live Ubuntu NIC was reset, unbound or reconfigured.

## Confirmed finding

`Controller::wait` checks `done()` before the operation's elapsed deadline.
A reset-clear read taking 100 ms or a PHYAR-ready read taking 50 ms therefore
returns success. Its global budget check is also only before that read.
The PHY-reset loop similarly accepts a clear BMCR before its 500 ms check.
Finite poll counts and the eventual 1500 ms startup failure do not establish
these advertised per-operation bounds. This is a P2 startup correctness bug;
it does not itself explain missing physical carrier on the Ryzen.

The implemented correction preserves the 50/100/500/1500 ms limits, checks before and
after each real observation, rejects a regressing monotonic clock, and never
continues negotiation or publishes DMA after an expired observation. The six new tests
invoke production `prepare()`, not a copied wait implementation.

## Native ownership and remaining hardware evidence

The native service requires MEM-only PCI command before reset, initializes
and programs rings before `platform::resume`, and retains the Device/DMA after
any resume attempt, including failure. Before that transition, failures can
drop the mapping only while bus mastering is off. Runtime errors request
controller stop and verified platform quiescence, keeping the same DMA object.
Normal service work is bounded (64 TX, 64 RX, 16 queued frames per turn).

An unplugged link is observed without blocking negotiation. A pending TX that
fails to complete for five seconds stops new work and remains unavailable
until supervised restart. This is the documented baseline, not demonstrated
automatic link/hang recovery. Cold-start PHY behavior, cable/replug, actual
DMA/MSI, DHCP, sustained traffic and recovery on the specific Ryzen remain
the open task's hardware criteria. Source review does not replace those runs.

## Correction and validation

Own-branch implementation `f5ee34b08374c3a8a6267d95817cdc7cce039c44` built
successfully as the native release RTL8168 executable. Independent read-only
review found no further blocking defect in this scoped delta. After raw
publication `bc802db7a0ce06aae8b93da538964b57a10fb3d7`, the actual
`rustc --edition=2021 --test tests/rtl8168_host.rs` binary passed all 22 tests:
0 failed, 0 ignored, 0 filtered, 0.00 s reported runtime. Configuration: x86-64
WSL2 kernel `6.6.87.2-microsoft-standard-WSL2`, Rust
`1.101.0-nightly (c36f14571 2026-10-01)`. Six new regressions
exercise production `prepare()` with exact/late reset and PHYAR reads,
just-before-cutoff completions, scheduler oversleep, PHY-reset settling sleep,
a late global-budget read and clock regression. Existing descriptor ownership
and bounded frozen-clock tests still pass. No test or build ran on fast-test.

H15's source defect is corrected. Task 213-DRV-0031 remains open for the exact
Ryzen hardware acceptance above; no physical Ethernet success is inferred.
