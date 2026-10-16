# 213-DRV-0032 — Bring up the Ryzen RTL8852BE Wi-Fi adapter

**Type:** driver · **Owner:** ASTRA auditor-fixer, for `DRV`, with `KRN` and `NET` integration · **Priority:** P1 (the maintainer's first network priority, 2026-10-15) · **Status:** open · **Blocked by:** [213-DRV-0033](213-DRV-0033-rtl8852be-firmware-and-probe.md), [213-KRN-0114](213-KRN-0114-realtek-driver-startup.md), [device-bound platform policy](requests-KRN.md#device-bound-platform-power-policy-for-the-ryzen-wlan-driver), hardware calibration and radio bring-up · **Main task:** [213](213-amd-ryzen-pc.md), [550](550-network-on-real-hardware.md) · **Roadmap:** tracks H, A, D · **Constitution:** MC-2.3, MC-3.3, MC-6.1, MC-12.1, MC-12.3, Appendix B.6

## Problem

The Ryzen report `hw0005.txt`, build e6730d7a, identifies `03:00.0`, `10ec:b852`, revision `00`, subsystem `10ec:b852`: RTL8852BE. There is no MIND driver for it. Existing VirtIO/Broadcom paths cannot make this adapter work. A firmware parser or a PCI identification line is not Wi-Fi support.

The maintainer requests Wi-Fi first, with RTL8168 Ethernet work in parallel, then tests/configuration through Effector. The existing WLAN service can supply association and WPA2 policy only after this adapter supplies working 802.11 management/data frames and channel control.

## Plan

1. Establish precise source/licence provenance, select a compatible official firmware, and collect the digital chip cut without RF/DMA initialization: [213-DRV-0033](213-DRV-0033-rtl8852be-firmware-and-probe.md).
2. Implement a bounded MAC power lifecycle for RTL8852B, including rollback and explicit power-state diagnostics. Add the documented indirect XTAL/EFUSE reads only after that sequence is reviewed. Read the real MAC address, RFE type, crystal trim, country and board calibration; never invent defaults from PCI identity alone.
3. Port the supported board's BB/RF/NCTL tables and calibration sequence from licence-compatible, pinned sources. Record every imported table and source in `THIRD_PARTY.md`. Respect country/regulatory power restrictions. Missing or unsupported calibration leaves transmission off.
4. Implement firmware download and H2C/C2H transport with absolute deadlines, bounded retries, secure-boot policy and explicit status. Validate a pinned, hash-verified image before any device write. Structural parsing alone is not authentication, approval of download addresses or secure-boot support.
5. Implement PCIe RX/TX descriptors, buffer ownership, DMA address validation and interrupts in a separate user-space driver. KRN reviews capability/IRQ/DMA integration; no ABI change is made in this DRV task. Link-down, firmware failure and malformed descriptors must not stall other network cards or tests.
6. Connect the driver to the existing WLAN service, initially the narrowest measured configuration: one supported band/channel width and WPA2-PSK, then scan/associate, DHCP, DNS and ping during test load. Extend capabilities only with evidence for each configuration.
7. Run the applicable host/QEMU gates on ASTRA, then record the actual Ryzen build, AP/band/channel, firmware hash, association/IP/link events and latency/loss during the test suites. Include reconnection after AP loss and bounded failure when firmware is absent. Effector registration/poll/report is a separate end-to-end check after stable IP/TLS exists.

## Acceptance criteria

- The actual Ryzen RTL8852BE associates with the configured AP, obtains an address and keeps ping plus ordinary traffic responsive while MIND tests run; report measurements, not only a successful initial ping.
- Firmware/board/calibration provenance and supported chip cuts are explicit. Unknown cuts, unavailable firmware and invalid calibration fail with useful diagnostics and without transmission.
- RX/TX and firmware-command paths have bounded waits and resource limits; interruption, link loss and restart do not corrupt ownership or block unrelated services.
- Firmware remains outside the repository and built images, in the maintainer's `proprietary/` store and, after explicit copying, `data/firmware/` on the written boot volume.
- Every implemented slice has focused tests; required project gates and hardware evidence identify their configuration. Documentation states limitations, including unsupported security modes/bands.

## Related

[213-DRV-0033](213-DRV-0033-rtl8852be-firmware-and-probe.md), [213-DRV-0031](213-DRV-0031-rtl8168-wired-network.md), [550](550-network-on-real-hardware.md), [750](750-effector-agent.md), [RTL8852BE source/format notes](../hwdocs/rtl8852be.md), `wlan/`, `wpa/`, `init/src/main.rs`.

## Progress (2026-10-15)

The first slice provides source pins, a firmware fetch/check helper and bounded, host-tested format/register helpers. It performs no radio, firmware-download, association or packet-transfer operation. The task remains open.

On 2026-10-15 at 18:34:35 UTC, the Ryzen's Ubuntu `7.0.0-38-generic` driver boot log established digital cut 1 (CBV), analog cut 1 and RFE 1 for `0000:03:00.0`, through Effector operation `20261015183435.268000692_000025`. No debugfs/MMIO access was needed. The compatible pinned normal-CE image is cut 1, version 0.29.29.15, offset 96, size 326616. This removes the unknown-cut/RFE selection question; it does not provide the remaining EFUSE/PHY calibration values or a MIND MAC power, firmware-download, DMA or radio implementation. The raw MIND `SYS_CFG1` observation remains in [213-DRV-0033](213-DRV-0033-rtl8852be-firmware-and-probe.md).

### Implemented slice: bounded native board access

The maintainer reaffirmed Wi-Fi as the first network priority after the x86
validation stage. Work proceeds on `codex/213-DRV-0032-rtl8852be`, from ASTRA
`1aea9100` merged with main `bd7b5493`. The slice is an executable
`rtw89_wifi` service with exact PCI/cut checks, the pinned RTL8852B MAC power
lifecycle, bounded indirect XTAL operations, board EFUSE reads and shutdown.
Tests exercise the same register sequence through a simulated I/O seam;
they cannot establish operation on the actual adapter.

The service must begin with memory decoding enabled and PCI bus mastering
disabled. The current generic BAR grant enables both, and no RTL8852BE boot
service exists. KRN-0114 supplies the required boot image and MEM-only BAR grant;
this task does not change the boot ABI or borrow the Broadcom service identity.
Firmware download, DMA rings, BB/RF calibration, WLAN registration and traffic
remain later work. No Linux driver unbinding, reset or radio experiment is part
of this slice.

Own-branch commit `a6e69a07` builds for `x86_64-unknown-none`. After its
publication in `origin/fast-test` merge `0b5bd1bf`, all 16 focused host tests
passed on 2026-10-15 (0.05 seconds). No tests ran on `fast-test`. Independent
review found and corrected the native 10 ms sleep-tick mismatch and acceptance
of transitional MAC power states. Short hardware waits now require the
calibrated clock; initial/final MAC power state must be zero. These results do
not establish hardware operation or a network link. KRN-0114 integration must
rebuild the executable with its final boot ABI; actual Ryzen board reads,
power-off and all radio acceptance criteria remain pending.

### Implemented slice: bounded PCIe FWCMD transport

Commit `f6caafca` implements channel-12 TXBD/24-byte TXWD packets,
installation while PCI mastering/HCI engines are off, register readback,
ordered descriptor publication, guarded hardware-index reclamation and a
bounded drain. A separate payload pool preserves the pinned Linux driver's
eight-completed-command retention requirement (`RTW89_PCI_MULTITAG`).
The initial native board service remains without DMA and does not call it.
MAC DLE/HFC/PCIe pre-initialization, hash-pinned firmware/WCPU sequencing,
RX/C2H and the KRN DMA integration are explicitly separate prerequisites;
the transport must not enable bus mastering or release DMA on its own.

The own-branch x86-64 native release build passed (0.34 seconds). Following
`fast-test` publication `dd735d0a`, all 14 focused command-queue tests passed
on 2026-10-15 at 21:01 UTC (0.03 seconds, no ignored tests). Independent
review checked the pinned descriptor/retention contract and found a missing
`0xffff` PCI Command guard, which was corrected before publication. The
implementation rejects non-x86-64 architectures and requires coherent WB
DMA with UC MMIO. Host fixtures do not establish hardware ordering or actual
DMA operation. The frozen earlier `a99cc44a` full gate does not contain this slice;
its full gate, integration and hardware acceptance remain pending.

### Implemented slice: MAC download-memory preparation

Commit `371d41db` ports the exact RTL8852B PCIe `RTW89_QTA_DLFW` WDE/PLE sizes and quotas,
including the SCC WCPU minimum quota override, followed by HFC channel-12
pre-cost/enabling. Require supported cut, active MAC, reset baseband, stopped
WCPU and disabled PCI mastering/HCI before any write. Bound readiness polls
and attempt independently budgeted HFC/DLE disable with explicit cleanup
errors on every modified failure. This slice prepares internal device memory
only; it neither enables host DMA nor replaces the still-required PCIe
MDIO/DBI/mode/BDRAM startup, firmware download or calibration.

The native x86-64 build passed (0.31 seconds, no warnings), and all 11 focused
host fixtures passed on 2026-10-15 UTC after `fast-test` publication `60a100be`.
Independent source review confirmed the quota/ordering/cleanup mapping and
identified a final-read deadline gap, corrected before publication with a
dedicated regression. The service-directory build passed after correcting an
initial root-directory invocation that selected the Linux target. This library
stage remains uncalled by the native board service. Actual hardware operation
and a full gate containing this later commit remain pending.

### Implemented slice: bounded PCIe preparation

Port the pinned RTL8852B-specific PCIe preparation through MDIO/DBI, link
workarounds, stopped DMA/index clearing and descriptor modes. Use only the
supported BAR2 DBI fallback for the named vendor bytes; do not widen the
kernel configuration-write interface. Restore the temporary L1 bit with a
fresh budget on every attempted change, preserving primary and restoration
errors. Keep PCI mastering and both HCI engines off. After command-ring
installation, configure the source's single-band BDRAM table and bound its
reset; leave all DMA channels stopped. No native call or DMA grant is added
until firmware sequencing, ring ownership and kernel quiescence are reviewed.

Own-branch `4feee434` built for native x86-64 (0.64 seconds, no warnings).
After `fast-test` publication `1dee0b3f`, all 16 focused PCIe host tests passed
on 2026-10-15 UTC (reported 0.00 seconds, no ignored tests). Independent
source review found no remaining blocking defect; actual hardware behavior
and a full gate containing this slice remain unvalidated.

### Implemented slice: hash-pinned firmware and WCPU download

Verify the entire private file against the documented immutable SHA-256 and
size before device writes; select only the measured cut-1 normal-CE image
and explicitly check its version, header and section address/length metadata.
This is local pinned-file integrity, not manufacturer-signature verification.
Permit only the pinned source's explicitly nonsecure OTP pair `ff/ff` at
physical `0x5ec`/`0x5ed`, already covered by the board's PHY-capability read;
other security states fail closed. Keep the file immutably borrowed while
sending it. Implement the pinned WCPU reset/start, header H2C, raw bounded
section chunks and readiness sequence through the real channel-12 queue.
Require reviewed DMA/ring activation beforehand, bound the whole attempt and
backpressure, retain the last eight command buffers, and poison failed
attempts while independently stopping HCI and WCPU. Do not retry or free DMA
without the separate quiescence contract. Native activation, complete MAC,
C2H/RX, calibration and the WLAN frame service remain separate integration.

Own-branch `cf768d22` (following `27033a43`) native build passed (0.39 seconds,
no warnings). After `fast-test` publication `45387d45`, all 18 synthetic
host cases passed (0.19 seconds), the explicitly requested private-file
hash/mutation fixture passed (0.05 seconds), and all 14 affected FWCMD tests
passed (reported 0.00 seconds) on 2026-10-15 UTC. Independent review fixed
final-drain deadline handling; actual source/kernel integration checks
corrected optional PCI I/O decoding and the existing powered-CMAC state.
No native activation, hardware firmware readiness, RF or network claim
follows from these tests. Required later integration/full gate remain open.

### Implemented slice: exact RFE-1/cut-1 PHY tables

The separate branch `codex/213-DRV-0032-rtl8852be-phy` provides the pinned
BSD BB/gain/RF/NCTL data and bounded production stages in `phy.rs`.
The RF stage includes both physical paths and mandatory firmware mirrors
under one deadline. Partial failures poison the attempt and independently
attempt BB/RF disable; guard refusal never changes an unknown active device.
The library requires actual firmware-init-ready, full single-band MAC,
BB access, stopped data paths and a complete BAR2 mapping. The native service
still does not call it. The chip TX-power unit, gain decoding and BB reset
now follow the BB table under the same deadline; failed interleaves cannot
publish gains or admit RF programming. The additional RCK stage follows NCTL and source DPK backoff, with bounded
analog/direct RF access and separate RF5-restore/chip-disable errors.
BTC/pre-NCTL DM interleaves, remaining RFK calibration,
board trims and regulatory authorization remain integration
work, as specified in [the PHY contract](../hwdocs/rtl8852be-phy.md).

The initial `59a77177` run passed 18/19 Rust cases; the RF-prefix case
incorrectly expected register 0 before the pinned table's first register 5.
After correction and publication of `f4766604`, all 20 Rust fixtures and four
extractor fixtures passed, as did pinned-source regeneration. Published
`78cf274a` passed all 24 Rust cases including the TX-power/BB-reset interleave.
Published `0d6c66bf` passed all 34 RCK/RF/PHY fixtures. The separate DACK
engine follows with mandatory typed BTC START/STOP, pinned BSD calibration
tables and preserved per-stage/cleanup failures. After publication of
`7c6d5ecc` and `a8418545`, all 44 Rust and three DACK extractor fixtures
passed, as did pinned table regeneration. The next initial RXDCK slice uses
the shared zero-only scheduler mailbox, post-DACK phase guard and actual BTC
contract. Published `abe309ea` passed all 56 Rust fixtures, three DACK
extractor fixtures and pinned regeneration. It never enables packet transmission.
Actual BTC arbitration and the pre-NCTL DM prefix still block native activation. No hardware or radio success is claimed.

Independent review corrected the generic-MAC BB/RF guard/cleanup to the actual RTL8852B common-chip dispatch before native activation. Cleanup now clears AFC/BB reset and both XTAL RF-enable fields without replacing a pending indirect command.

### Contiguous DM prefix (2026-10-16)

The private `phy::dm` engine now follows the entire initial pre-NCTL register
prefix, with a 300 ms total deadline and independent 400 ms chip-disable
cleanup. It returns initial observations/settings and keeps the public pipeline
closed until actual BTC `ntfy_init` completion supplies its concrete token.
Native build passes; after publication `badae1bc`, all 68 fixtures of `727aab95` passed (0.11 seconds, none ignored).
The [PHY contract](../hwdocs/rtl8852be-phy.md#contiguous-initial-dm-prefix-and-remaining-order)
records exact skipped chip hooks and the remaining source order through board
trim, actual firmware PHY capabilities, channel calibration and association.
Owned active-RX permits and actual BTC policy remain integration dependencies;
no fake readiness callback, radio activation or hardware success is supplied.


### Implemented library slice: single-band MAC preparation after firmware readiness

Implement the actual RTL8852B chip-dispatched BB/RF enable (AFC/SPS/XTAL/XYN),
then source-derived `sys_init_ax`, SCC DLE/HFC and DMAC/CMAC0 preparation.
Stop all packet scheduler queues through the firmware register mailbox, keep ordinary host DMA stopped, retain the
command queue and verify real firmware status before writes. Bound CAM,
scheduler and DLE readiness; a partial failure poisons the attempt and
independently disables chip RF/BB and command DMA/WCPU without freeing DMA.
This prepares the configuration needed by the reviewed PHY stages; it must
not claim calibrated RF, usable RX/TX or a complete WLAN link. Packet report
routing/interrupts, normal H2C sequence/C2H processing and final offload setup
remain explicit subsequent runtime integration, not silently omitted from a
full-MAC guarantee. Native main/init are owned by the separate integration
agent and are not modified here.

The firmware-ready scheduler path follows `rtw89_hw_sch_tx_en_h2c`: refuse
occupied/foreign register mailboxes, send SCH_TX_EN for CMAC0/all queues,
check the TX_PAUSE response and actual scheduler/protocol idle, and never
replace or retry a pending command during cleanup. Hardware diagnostic
counter nibbles retain the sole-writer contract. The whole preparation is
bounded by two seconds; failure separately checks packet stop, disables
chip BB/RF, and stops command DMA/WCPU while retaining the primary error.
The packet-report/interrupt and final runtime H2C/C2H work remains open.
Own-branch `02b6b4e3` native build passed (0.43 seconds, no warnings).
After `fast-test` publication `8461fd20`, all 14 focused runtime-MAC host
fixtures passed (0.12 seconds, no ignored tests), on 2026-10-15 UTC.
Independent review added the live CMAC-enable prerequisite; no blocking
source-review finding remains. Hardware validation and a full gate
containing this library slice remain pending.

### Implemented library slice: bounded RXQ/RPQ and C2H parsing

Implement the exact RTL8852BE RXBD layout and chip flags from the pinned
PCIe source: 256 descriptors per RXQ/RPQ, 11498-byte buffers, normal
empty-equal indices, FS/LS present and no RX-tag check. Use a retained,
disjoint coherent DMA region within a future 8 MiB allocation, without
changing the current 128 KiB native firmware diagnostic grant. Install
only while bus mastering/RXHCI are off; activation validates already
installed rings and never releases an uninstalled DMA channel.

Each receive call processes at most one descriptor under the caller's
absolute budget, copies volatile DMA bytes into caller storage before the
16-bit host-index release, and never allocates per packet. Reject invalid
indices/lengths and partial/fragmented frames explicitly; preserve buffers
on a possibly applied doorbell error. Decode short/long AX RX descriptors,
RPQ reports and complete bounded C2H headers, distinguishing receipt ACK
from done ACK and matching category/class/function/sequence. This is not
interrupt handling, full H2C/C2H integration, BTC arbitration, association
or WLAN registration. Add meaningful malformed-data/ownership/wrap/budget
host fixtures before native integration; no live Linux NIC access.

The RX region occupies 5,902,336 bytes (4096 descriptor bytes plus 512
64-byte-aligned buffer strides of 11520), disjoint from the 86,016-byte
command region. The combined layout fits within a future 8 MiB retained
grant. Own-branch `b112406d` native build passed (0.47 seconds, no warnings).
After raw publication in `fast-test` `cfc4515bf1d1164c75a11ecd959aaca05326d88c`,
all 18 RX host fixtures passed on 2026-10-16 (0.13 seconds, none ignored).
Independent source/ownership review found no remaining blocker. Hardware
validation and a full gate containing this slice remain pending. The current firmware
diagnostic's grant and native entry are unchanged.

A warm inherited device remains an explicit integration precondition:
PCI MEM-only does not imply MAC power state zero or ownership of previous
DMA buffers. The current board guard continues to refuse nonzero MAC power
state. A future owned warm-state shutdown/reset must establish quiescence
and retain any previously reachable memory before cold reinitialization;
this slice does not pretend to provide that transition.

### Initial normal-H2C and BT-arbitration prerequisites

`h2c.rs` adds normal sequence/header construction, a real command-queue
adapter and bounded receipt/DONE correlation through `c2h.rs`. Eight slots,
one-second maximum absolute deadlines and a first-stage 256-command limit
bound state. Full before publication consumes no sequence; ambiguous sends,
time regression and timeout poison the session without retransmission.
Unrequested/mismatched ACKs cannot turn a command into success. Continuous
sequence reuse, RX activation and native command routing remain separate.

`btc.rs` implements an actual bounded read-only BT RFK RUN/REQ wait. Its
snapshot is not an RFK grant: DACK START/STOP still needs initialized BTC
context, an owned WLAN scoreboard shadow, antenna/grant configuration,
version-matched TDMA policy and restoration/cleanup. The BT read view cannot
be used as the WLAN write shadow. No fake successful callback is supplied.
The exact source flow and next integration boundaries are in the
[hardware notes](../hwdocs/rtl8852be.md#normal-h2c-sequencing-and-bt-rfk-observation-prerequisites).
Own-branch `d951d101` builds natively (0.53 seconds, no warnings). After
raw publication `d9266c7b`, all thirteen H2C and six BT-observation fixtures
passed on 2026-10-16 (both 0.00 seconds, zero failed/ignored; x86-64 WSL2,
rustc 1.101.0-nightly c36f14571). Independent reviews corrected two clock
and deadline boundaries before publication. The native service and existing
firmware-download path are unchanged. Hardware acceptance stays open.

### Initial calibration scheduler contract

Expose the existing firmware-owned scheduler-zero exchange for the initial
RXDCK wrapper. Both pause and restore are restricted to an already-zero
saved scheduler mask, with live firmware/CMAC guards, exclusive register
mailbox ownership, caller timeout and actual CTN/PTCL idle readback. No
nonzero TX scheduling is enabled. A failed or uncertain exchange forbids
further mailbox calls in that session; callers must retain DMA and perform
their independent shutdown. Add focused zero/occupied/deadline fixtures;
native calibration and BTC arbitration are still separate prerequisites.

Own-branch `b947aa40` built without warnings in 0.46 seconds. After raw
publication `d9266c7b24ba3f3946c32e3ba56592a8b5ad1769`, all 17 runtime-MAC
host fixtures passed (0.12 seconds, none ignored; 19 unchanged downloader
cases filtered). The exact commit ran on the owned scheduler-test branch
before the later post-init code was published. Independent review found
no blocker; native RXDCK/BTC integration and hardware evidence are pending.

### Runtime MAC tail and installed receive-channel activation

Implement the pinned RTL8852B MAC error-mask/host-report tail and PCIe
LTR/address-info setup, then activate only the installed RXQ/RPQ and
existing command channel. Require the previous MAC preparation, matching
unchanged ring descriptors, quiet TX scheduler, ready firmware and disabled
host interrupt masks before any writes. BACAM V0 makes this chip's
`mac_feat_init` a no-op; do not send another chip's V1 BA-CAM command.
Preserve STOP on every uninstalled data-TX channel and WPDMA. Keep host
interrupts masked for the bounded polling path. A failure independently
stops receive DMA, disables the radio and stops command/WCPU operation;
retained allocations still require external PCI quiescence. Add applied
write-failure, stale-ring, status-bit preservation and deadline fixtures.
No native grant expansion, normal H2C/BTC/PHY claim or WLAN registration
is included in this slice.

Own-branch `06a0b932` built without warnings in 0.66 seconds. After raw
publication `b50f52c0`, all nine focused post-init fixtures passed (0.81
seconds, none ignored; 36 other module cases filtered), as did all 18
affected RX fixtures (0.12 seconds, none ignored). The source/mask/ownership
review found no blocker. These checks ran on 2026-10-16 MSK; a full gate
containing this slice and actual hardware operation remain pending.

### Hardware BTC initcfg prerequisite

Implement bounded RTL8852B/RFE-1 PTA/LTE, WLAN priorities, shared-antenna RF
LUT and counter initialization between actual `Phy::RadioLoaded` and NCTL.
`ConfigPrepared` deliberately represents only this hardware stage; actual
BTC software context, scoreboard ownership, report/control policy, antenna
selection and RFK START/STOP restoration remain open. The existing live
PHY guard continues to reject active ordinary RX/TX. A failed modified
attempt independently closes both LUT write gates, disables BB/RF and
poisons the PHY state, retaining primary/cleanup diagnostics and DMA.
The native service is unchanged. Own-branch `e6ed1e2c` native build passed
(3.53 seconds, no warnings). After raw publication `c2facb84`, all nine
new phase/source-order/failure/deadline fixtures passed on 2026-10-16. The
executable also ran 18 inherited synthetic downloader cases successfully
and retained its one existing optional private-file ignored case: 27 passed,
zero failed, one inherited ignored, 0.19 seconds. No hardware or working-radio
claim follows from this library.

### Typed receive-session revalidation

Bind `Receiving` to the original command and receive objects with unique,
nonzero process-local session numbers; never wrap or reuse an exhausted
number. Add a bounded public read-only validation that checks those objects,
running state, real DMA addresses/counts/indices, firmware and MAC state,
RX-only DMA configuration, stopped packet TX, and masked host interrupts.
Return an opaque permit borrowed from the token and both rings, not a bool.
The future PHY adapter may request this permit for an owned receive path;
the existing stopped-RX guard is unchanged until that integration. Test
same-address pool reconstruction, stale/foreign tokens, partial live traffic,
actual register corruption and timeout without publishing MMIO writes.

Own-branch `b8f41cb5` built without warnings in 0.51 seconds. After raw
publication `767c2cb8`, all 16 post-init/session fixtures passed (0.82
seconds; 36 unchanged module cases filtered), all 18 affected RX fixtures
passed (0.12 seconds), and all 14 FWCMD fixtures passed (reported 0.00
seconds), with none ignored, on 2026-10-16 MSK. Two independent read-only
reviews found no blocker. These are host model results; full-gate,
physical-device operation and the PHY/native permit consumer are pending.

### Firmware-ready command-session binding

Bind the downloader's opaque `Ready` result to the successful command
queue's existing non-wrapping session identity. Require that exact identity
in `runtime_mac::prepare` before hardware access, while retaining all live
register checks. A stopped or reconstructed queue cannot reuse an old
firmware token. Add host regressions using an actual synthetic downloader
completion, including old-token/new-queue rejection at the same physical
base. Future BTC context must also bind its PHY/init attempt and revalidate
live receiving state; a matching firmware token alone is not an RFK grant.

Own-branch `383b27e5`, based on consolidated ASTRA `1b6518e3`, built without
warnings in 3.31 seconds. After raw publication `badae1bc`, all 19 runtime
MAC cases passed (0.12 seconds) and 18 synthetic downloader cases passed
(0.19 seconds). The separately fetched private-file hash/mutation fixture
also passed when explicitly invoked (0.06 seconds). The cumulative private
fixture passed GEN1/GEN2 download and the missing-H2C case with real pinned
file bytes and modeled board/PCI/DMA, on 2026-10-16 MSK. Independent review
found no blocker. Full-gate and hardware validation remain pending.

### PHY consumer of the owned RX permit

The shared PHY guard now accepts active RX only through the actual borrowed
`ReceivePermit`, with default refusal, a 10 ms callback ceiling and repeated
DMA/packet-TX checks after validation. Stopped-RX behavior and exact before/after
DACK phases remain distinct. Six integration fixtures exercise real session
objects and stale/current hardware state without fabricating a token; the
PHY, BTC-init and post-MAC host harnesses pass 125, 66 and 57 cases respectively
after `2a41a980` was published in `08a4c784`; all six new cases pass in each.
These harnesses repeat shared cases and each retains the existing ignored
private firmware-file fixture. Native build passes. This does not grant BTC
or transmit readiness.


### Board calibration decoder

Add a pure cut-1/RFE-1 decoder for logical TSSI/thermal/gain fields and physical
power-calibration, TSSI/thermal/PA trims and gain compensation. Preserve exact
packed offsets, signed nibbles and source global sentinel rules; never replace
missing firmware PHY-capability replies with these OTP bytes. Nine fixtures
compile and native build passes; execution awaits publication. Register the
mandatory calibration harness in host CI. No register write or native PHY
activation is introduced by the decoder; DM tail and channel work remain open.

### PHY attempt binding for BTC integration

Give each PHY attempt a bounded nonwrapping identity independent of its bus
address. Allocation exhaustion creates Failed before MMIO. The BTC owner can
bind ConfigPrepared to this identity and actual firmware/ring sessions without
a raw bool or pointer identity. Two model fixtures cover same-address reuse and
exhaustion; test execution awaits raw publication. No native activation changes.

### MAC report tail after the full PHY prefix

Implement the pinned single-band AX PPDU status/report forwarding and RTS
threshold writes as a crate-private bounded stage. The AX PHY-report hook
is null and requires no substitute command. Require a fresh actual owned
receive-session permit and post-DACK register state; the eventual caller
must also establish complete BTC/PHY/RFK/power-trim/path prerequisites.
Never treat this stage as WLAN or regulatory readiness. On a possibly
applied write error, independently disable PPDU reports, stop RX, disable
BB/RF and stop command DMA/WCPU, preserving all errors and retaining DMA.
Add exact mask/order, threshold boundary, stale-session, applied-failure
and final-deadline fixtures without enabling native operation.

### Board calibration decoder

Add a pure cut-1/RFE-1 decoder for logical TSSI/thermal/gain fields and physical
power-calibration, TSSI/thermal/PA trims and gain compensation. Preserve exact
packed offsets, signed nibbles and source global sentinel rules; never replace
missing firmware PHY-capability replies with these OTP bytes. Nine fixtures
compile and native build passes; execution awaits publication. Register the
mandatory calibration harness in host CI. No register write or native PHY
activation is introduced by the decoder; DM tail and channel work remain open.

### PHY attempt binding for BTC integration

Give each PHY attempt a bounded nonwrapping identity independent of its bus
address. Allocation exhaustion creates Failed before MMIO. The BTC owner can
bind ConfigPrepared to this identity and actual firmware/ring sessions without
a raw bool or pointer identity. Two model fixtures cover same-address reuse and
exhaustion; test execution awaits raw publication. No native activation changes.

### BTC command/profile and transition prerequisites (2026-10-16)

The next slice adds exact firmware-profile-105 encoders for monitor, slots,
report/control/init/role/AFH and no-link/RFK policy, plus bounded owned
scoreboard, antenna/grant snapshot/restoration and cold-init common BB
operations. Generic BTC DONE and direct CXDRVINFO ACK behavior stay
separate through production H2C. Own-branch `94762996` no-std build passed
(0.80 seconds); after raw publication `1006c136`, all seven codec and eleven
control source/format/fault fixtures passed (0.00 seconds each, zero failed
or ignored). Source reviews by the PHY and MAC/RX owners found no blocker.
These helpers cannot construct an Initialized or RFK-grant result. The
session-bound owner context, full ordered C2H/report and common-action
orchestration, calibrated PHY interleave and native activation remain open;
see [precise scope](../hwdocs/rtl8852be.md#exact-coexistence-command-codecs-and-bounded-register-transitions).

### Report-tail execution evidence

Own-branch `150f5b80` built without warnings in 0.71 seconds. After raw
publication `1006c136` (including PHY-capability tip `a877030f`), all seven
report-tail fixtures passed in 0.27 seconds and all 16 affected post-init/
session fixtures passed in 0.88 seconds, none ignored, on 2026-10-16 MSK.
Independent source review found no blocker. Native/full-gate/hardware
operation remains pending.

### Actual firmware PHY capability query

Add the pinned AX GET_FEATURE register exchange before runtime MAC setup,
using the scheduler's existing exclusive mailbox and counter implementation.
Require the downloader's actual Ready token for the same running command
queue and the live DLFW state; preserve all four response words and distinguish
firmware fields from the source's successful-zero-field NSS fallback. Only a
complete matching response may create a session-bound capability result.
Missing, malformed, late or foreign-session responses cannot imply two NSS.
Retain an uncertain mailbox and DMA until independent command/WCPU cleanup;
never retry a failed exchange. Test the real exchange, counter rollover,
source resolution, busy/malformed/frozen-clock cases and independent cleanup.

Own-branch source `1c854cda`, identity-consumer API `2629a614` and test-only
production-query reuse helper `a877030f` built without warnings at the final
tip in 0.73 seconds. After raw publication `1006c136`, all 10 PHY-capability
fixtures passed in 0.14 seconds and all 19 affected runtime-MAC fixtures
passed in 0.13 seconds, none ignored, on 2026-10-16 MSK. Independent source
review found no blocker. These are synthetic device results, not a firmware
response observed on physical hardware; native integration remains pending.

### BTC 105 receive-envelope prerequisites

Decode complete bounded C2H envelopes for the exact pinned BTC 105 profile,
including the source's legacy report/event ID mapping. Validate the whole
report stream before exposing borrowed typed TDMA-v3, slots-v1 and BT-version-v1
records; preserve unsupported records/events explicitly. BTINFO and scoreboard
notifications must reach the session owner, not disappear as ignored traffic.
The pure decoder neither correlates a report with a command nor grants RFK;
the session owner must validate provenance and actual applied-policy contents.

Source `96941305` and host-gate registration `3b0ca3d4` built on the own
branch. After raw publication `539b588e`, all seven decoder fixtures passed,
none ignored, on 2026-10-16 MSK. [Stage evidence](../hwdocs/rtl8852be-btc-reports.md)
records the configuration and limits; actual firmware reports and policy
application remain pending.

### One-device cold firmware/MAC/RX model

Extend the existing cumulative cold OTP/power/PCIe/firmware model through
the real PHY-capability mailbox query, runtime MAC setup, installed RX
activation and fresh Receiving validation. Use the native disjoint 128 KiB
command reservation and remaining receive region inside one retained 8 MiB
allocation, one Board and one Queue/Ready session. Register the harness in
the normal host gate. Model device reactions to real writes, without
reseeding stage guards; exercise missing replies, runtime readiness failure,
failed RX installation/activation and independent shutdown. This model is
not evidence of physical firmware timing, RF calibration or WLAN traffic.

### Pinned firmware default-table provenance

Validate the complete MFW directory and source-aligned appended-element
boundary before accepting the exact pinned file. Its last and maximum image
end equal EOF; no appended firmware power/country override is present.
Carry this checked result through Verified and successful Ready into an
opaque default-table provenance value bound to the same command session.
Keep the public hash/board policy unchanged; no constructor may attest other
pins. Test malformed/unordered/trailing directories and old/new sessions.
This does not establish platform country/SAR/EIRP constraints or authorize TX.
See [metadata, API boundary and evidence](../hwdocs/rtl8852be-firmware-tables.md).

### Cumulative normal command and receive ownership

Extend the same cold-device model beyond firmware/MAC/RX into the production
exclusive Session. Consume normal channel-12 headers only while actual RX
and command DMA are enabled and ordinary TX remains stopped. Produce ACKs
through the programmed RX descriptors; preserve one sequence across RF
mirror and BTC commands, without reseeding guards or replacing production
correlation. Exercise missing/foreign/rejected/malformed ACKs and DMA that
does not report consumption after an ACK. Failures must prevent new commands
and retain the allocation until independent shutdown. This is host model
coverage, not observed firmware semantics or working WLAN.

### Post-RXDCK initial DM tail

Implement the source-contiguous power-reference, board thermal/PA trim and
receive-path/NSS sequence, using actual session-bound firmware capabilities
and DM gain baselines from this same PHY attempt. Keep it crate-private until
actual BTC and the full source startup ordering are integrated. Detailed
scope, failure behavior and pending evidence: [PHY tail](../hwdocs/rtl8852be-phy-tail.md).

### Owned normal H2C/RX startup session

Add an adapter that consumes the actual successful downloader token, retains
exclusive command/RX/Receiving/scratch ownership and preserves one normal H2C
sequence across PHY mirror and subsequent BTC commands. Enforce one absolute
100 ms maximum command budget, actual ACK correlation and final DMA drain;
uncertain failures reject subsequent PHY stages while leaving raw cleanup
accessible. Retain ordered BTC C2H messages in bounded copied storage and fail
on overflow. This prerequisite deliberately cannot issue Initialized or RFK
permission. Thirteen focused fixtures compile; execution follows publication.
The complete BTC policy/common-action owner and native activation remain open.
See [session contract and limits](../hwdocs/rtl8852be.md#exclusive-startup-command-and-receive-session).

Own-branch session commit `3ee6dd54` built for x86-64 without warnings in
1.09 seconds. After raw publication `539b588e`, all 13 session fixtures passed
in 0.08 seconds on 2026-10-16 MSK (zero failed/ignored, x86-64 WSL2).
The two existing runtime-MAC identity tests using the changed downloader helper
also passed separately (0.00 seconds each). Other inherited fixtures were
filtered from this focused run. Independent review found no remaining blocker
in this slice. These seeded-handoff models do not establish cumulative device
behavior, complete BTC initialization, RFK permission or hardware connectivity.

### Channel policy and pinned power-data preparation

Add the source-ordered [channel/RFK plan](../hwdocs/rtl8852be-channel-plan.md)
and pure pinned default power/country decoder. Preserve unknown policy,
firmware overrides and platform constraints as explicit dependencies; no
channel permission, native MMIO or transmit activation is added. See the
linked document for exact table provenance and pending/executed evidence.

### Pending XTAL command ownership during board cleanup

[Audit H14](../issues-audit/2026-10-16-xtal-cleanup-audit.md) identifies an
early cleanup gap outside the later native power-off guard. Both shared XTAL
read/write helpers now require the previous command to become idle within
50 ms and the caller's tighter budget before issuing another. Stuck/unavailable
commands remain untouched; board cleanup keeps its own timeout alongside the
primary fault. Focused regression and cumulative scenarios remain required
after raw publication. This does not add warm-device reset or hardware evidence.

### Owned BTC normal initialization and common actions

Add actual session/PHY-attempt-bound normal/no-link initialization through
initcfg, source H2C commands, antenna/grant/pollution controls, dynamic BTINFO
and scoreboard state, full relevant common actions and final live receive
validation. Optional policy reports are reconciled against all TDMA/slot fields
at an explicit command-completion boundary; missing optional reports do not
invent an extra firmware requirement. One aggregate budget and independent
failure cleanup prevent partial completion from issuing Initialized. This token
is not an RFK grant: actual START/STOP arbitration, remaining calibration,
native integration and hardware evidence remain open. Thirteen new owner and
two identity fixtures compile; execution follows raw publication. See the
[contract and report limits](../hwdocs/rtl8852be.md#owned-normalno-link-btc-initialization).


Owner evidence after raw publication `1a62e46f` / code `8b2caed7`: all 26
Session/owner, 11 initcfg and 11 control cases passed on the own branch with no
failures or ignores. Native build passed without warnings. The normal-mode
startup library boundary is tested in the seeded model; real RFK arbitration,
native integration, remaining calibration and hardware connectivity stay open.


### Actual owned RFK coexistence callback

Implement typed DACK/RXDCK START/STOP over the completed BTC owner: bounded
actual BT arbitration, source policy/antenna/common actions, scoreboard and
report reconciliation, and final owned RX validation precede permission.
Retain partial START until matching STOP. Poisoned H2C or disabled BB/RF permits
only bounded raw ownership cleanup with explicit incomplete-policy failure;
ordinary TX remains stopped and all DMA stays owned. Eleven new fixtures compile;
execution follows raw publication. Native binding, remaining calibration and
physical-device evidence are still open. See the
[RFK contract](../hwdocs/rtl8852be.md#owned-calibration-coexistence-start-and-stop).

### Initial channel register stage (2026-10-16)

The private [2 GHz/20 MHz channel stage](../hwdocs/rtl8852be-channel-stage.md)
adds source MAC/BB/RF/power sequencing and bounded failure cleanup. It requires
actual same-session firmware default-table provenance and explicit platform
policy; it has no native caller and creates no TX authorization. Channel RFK,
actual policy/board observations and physical traffic validation remain open.


### Shared IQK/DPK and nested coexistence prerequisite

Extend source-valued callback types for IQK/DPK and require actual held outer
IQK permission for matching RF_AB one-shots. Preserve partial nested admission
for matching cleanup; refuse orphan, duplicate, foreign and premature outer
release. TSSI receives an explicit stronger enclosing IQK START/STOP requirement
as a deliberate stronger admission rule, instead of copying upstream's ignored rejection.
Eight regressions compile; execution follows publication. Host IQK/TSSI/DPK
algorithms, their policy-bound TX prerequisites and native integration remain
open; [semantics](../hwdocs/rtl8852be.md#iqkdpk-and-nested-iqk-notices).

RFK/nested execution evidence: after raw publication `8ebd2da170902c5679ff5450e674007dd1dfb9ee`
containing code `a134b5a0` and `a6df7c83`, the own source branch passed all 45
Session/owner/RFK/nested cases (0.18 s) and all 11 control cases (0.00 s), with
zero failures or ignores. Native build passed in 3.24 s without warnings.
These seeded-device fixtures cover actual production owner/session logic;
hardware calibration, policy-authorized RFK transmission and native networking
remain open.

### Actual calibration scheduler adapter

Native composition exposed a missing `Session: RxdckIo` implementation. Add
the owned adapter to the existing `runtime_mac::scheduler_zero` exchange with
one absolute budget, live firmware/queue/RX checks, final elapsed check and
terminal poison on uncertainty. Seven production-entry fixtures compile;
native build passes. Execution follows raw publication, and this does not
close calibration or hardware acceptance.


### Native initial PHY and owned coexistence continuation

Join the verified firmware/MAC/RX startup to actual Session-based H2C, BTC
normal/no-link initialization, initial DM and NCTL/RCK/DACK/RXDCK, board trims
and the final report registers. Consume Ready exactly once; never replace
BTC permission or scheduler mailbox responses with successful placeholders.
Return observations only and always run the existing independent native
shutdown. Channel IQK/TSSI/DPK, actual regulatory/SAR admission, association
and hardware traffic remain open. All five cumulative cold-device cases passed after publication `b169a275`
(0.08 s); the twelve fixtures of the fifteen-stage cleanup matrix also passed.
See [the diagnostic boundary](../hwdocs/rtl8852be-initial-phy.md).


### Initial channel RXDCK and host IQK preparation

Implement the pinned initial non-DBCC 2 GHz/20 MHz wideband IQK engine, including
source register backups, bounded NCTL completion, LOK retries and both TX/RX
group paths. A private wrapper requires the actual same-attempt BTC owner,
channel token and separate calibration-transmit authority, runs production
channel RXDCK first and returns an opaque identity-bound result for TSSI.
No native authority provider exists yet. Unknown NCTL completion forbids another
trigger and forces independent radio shutdown before coexistence release.
Meaningful register/timeout/cleanup fixtures and source-table regeneration will
be registered in the host gate; execution follows commit/build/raw publication.
Physical quality, full channel RFK coordination and hardware traffic stay open.

## Initial TSSI continuation (2026-10-16)

The [private TSSI engine](../hwdocs/rtl8852be-tssi.md) implements both source
PMAC calibration samples on both paths, board thermal/DE tables, actual
owned BTC notices and bounded cleanup. It requires the actual IQK result
and a consumed engine-specific calibration authority; there is no native
provider until raw PMAC/RFK power and platform policy are resolved. Native
build passes. After raw publication `b169a275` of code `983e4c72`, the exact
owner PHY harness passed 183 cases (twelve new TSSI cases), zero failures and
one existing private-firmware ignore; five extractor cases and exact pinned
table regeneration also passed. These are model and source-integrity results.
This does not complete channel RFK, association or physical WLAN evidence.

Scheduler evidence for `ea66207f`, after raw publication `180f9559`: all 52
combined Session cases passed in 0.24 s on the own branch with no failures or
ignores. The seven new register-mailbox regressions are included; hardware
calibration, association and normal traffic acceptance remain open.

### Private initial host DPK

Implement the source initial 2 GHz/20 MHz, non-DBCC, both-path DPK engine and
same-session/channel/policy entry after actual BTC initialization and TSSI.
Consume separate DPK calibration-transmit authority; no native provider exists.
Actual BTC/scheduler callbacks, bounded NCTL/AGC, source backups and independent
cleanup retain primary and secondary errors. Unknown NCTL completion switches
radio off before mailbox cleanup and skips unsafe register restoration.
Fourteen engine fixtures and two provenance cases are registered; the
execution and first-run correction are recorded below. Full typed cold-start
integration, RFK power policy, hardware quality and association remain open. See the
[DPK source and bounds](../hwdocs/rtl8852be-dpk.md).

### DPK first-run cleanup finding (2026-10-16)

Published `2fc291a5` passed 13/14 focused DPK models; the final-STOP model
failed. Both Python provenance cases and exact source-table regeneration
passed. Review of that failure found redundant AFE/KIP/RF restore after a STOP
that may already have released the grant. The correction allows only emergency
radio shutdown after an attempted STOP, records skipped/unknown DPD disable
explicitly, and never repeats STOP or restores calibration state. The failed
run remains recorded in [DPK notes](../hwdocs/rtl8852be-dpk.md); corrected tests
passed 14/14 on the own source branch at `40addff0`, after build and raw
publication `c38df1dd` (0.01 s; no failures or ignored cases). Hardware
acceptance is unchanged and remains open.

### Initial channel coexistence callbacks (2026-10-16)

The owned Session now has source `conn_rfk` lifecycle, conditional actual A2DP
30 ms preservation and first-slot RTL8852B MCC notification callbacks, described
in [connection RFK notes](../hwdocs/rtl8852be-connection-rfk.md). They use the same
initialized context, normal H2C sequence and retained RX, refuse pending RFK,
preserve errors and allow a matching host END even after poisoning. Ten new
Session model cases and the existing 52 passed together on the own branch
at `bf6747f5` after native build and raw publication `c38df1dd`: 62/62,
0.29 s, no failures or ignored cases. The channel
coordinator must still validate real IQK/TSSI/DPK results and transmission
admission. No native RFK-power provider or working WLAN is claimed.

### Initial channel calibration coordination (2026-10-16)

The private [channel RFK continuation](../hwdocs/rtl8852be-channel-rfk.md)
connects actual same-session RXDCK/IQK, conditional A2DP windows, TSSI, DPK,
connection end and final MCC notification. Three consumed engine-specific
power authorities and live owner/PHY checks are mandatory. There is no native
power-authority provider or packet-transmit activation. Seven cumulative cold
model cases compile, preserving the existing five initial-PHY cases; execution
follows raw publication. Physical RF calibration and WLAN acceptance stay open.

### Raw MPDU transmit ownership and status prerequisite

Implement exact RTL8852B TXWD/WP/address descriptors and bounded retained DMA
queues for band-zero management and four ordinary access categories. Use a
fixed legacy rate, no aggregation or hardware sequence replacement, a selected
hardware limit of one and no RTS. Count actual DONE/RETRY_LIMIT RPQ-completed
descriptors separately from DMA consumption; bound resubmission to 1..15
descriptors with the original sequence and Retry bit. The exact-chip
transmission-versus-retry limit semantics remain unresolved, so preserve raw
status and explicit `AttemptCount::Unknown`; the exact-attempt `wlanhw` 1.1
adapter stays closed until that boundary is resolved.
Lifetime/MACID drops cannot manufacture a transmission count. Preserve
ownership on uncertain publication, malformed reports, deadlines or stop
failures. No native caller or IDL/ABI change is part of this library stage;
activation still requires actual channel/calibration/policy and CMAC setup.
Register meaningful descriptor, ring, status and cleanup fixtures in the host
gate; run them only after own commit/build and raw publication.


### Shared live initial-PHY continuation

Extract the existing ordered native diagnostic initialization into a private
`prepare` function retaining actual BTC/DM/path state on the live PHY session.
The diagnostic still shuts down; channel cumulative tests use the production
function, including its owner validations, rather than duplicating startup.
After publication `7ec8fbee`, source `ed63d767` passed five initial-PHY and
seven cumulative channel/RFK cases (0.08 s each; no failures/ignores). The
consolidated x86 build passed in 2 s. Native channel admission and station/service
activation remain separate open work.

### Correlated final firmware TX reports

Request actual special C2H reports and retain each TX payload until matching
RPQ, PCI consumption and final firmware status have all arrived. Follow the
pinned final-report rule (DONE or configured counter reached), cycle the
four-bit report identity only after completion, and fail terminally on timeout
or contradiction. Preserve the raw firmware count without equating it to an
unverified on-air attempts contract. An owned RXQ polling entry returns other
C2H/data packets explicitly; it cannot silently steal the session's traffic.
The firmware's ordered terminal-report behavior is an explicit TCB assumption,
consistent with the upstream driver's tag reuse; arbitrary firmware replay
is outside that contract. Native activation remains closed.

### Checked initial calibration profile (2026-10-16)

The private [calibration provider](../hwdocs/rtl8852be-calibration-policy.md)
resolves the actual supported Board/source profile and explicit country/platform
inputs, distinguishes checked absence from unknown/invalid SAR, and checks the
source TSSI HT20 MCS7 maximum request of 12 dBm. It binds the exact written policy
and OTP fingerprint to the current PHY/firmware attempt and issues three consumed
engine-specific permissions once, after actual live ownership validation. IQK/DPK
use the supported source sequence without inventing a numerical output guarantee
or a new certification prerequisite. Source `202bba96` passed all fourteen focused
cases, including the actual cold coordinator, and all nine affected complete host
harnesses after own build and raw `882bd735` publication (2026-10-16; inherited
private-firmware ignores unchanged, detailed counts in the provider notes). The native scoped
input depends on KRN's platform-policy request; physical Wi-Fi acceptance remains
open. This provider does not start native transmission.

### Initial station context continuation (2026-10-16)

The [private station continuation](../hwdocs/rtl8852be-station.md) prepares
source NO_LINK MACID0/port0/CAM0 after the actual channel calibration result,
using the same command/RX owner and actual board MAC. Indirect-table access
requires the full aperture, commands have bounded actual ACK/consumption, and
modified failure independently disables radio/port and retains DMA. Eleven cold
continuation fixtures compile; execution follows raw publication. Association,
receive filtering and native WLAN/TX activation are not established by this
initial context.


### Raw receive-frame service boundary

Add the private [receive serializer](../hwdocs/rtl8852be-frames.md) for complete
legacy-rate MPDUs: retain the original protected payload, reject hardware error,
decryption and unsupported aggregation, remove the source-included FCS and
encode the existing wlanhw 1.1 prefix. Signal remains explicitly unmeasured.
The actual runtime filter/channel epoch is a caller obligation. Five focused
cases compile; execution follows build and raw publication. This does not
activate the endpoint or establish physical reception.

### One retained runtime RX owner

Keep `Session` as the exclusive reader of both receive rings during H2C waits.
Add an explicitly caller-owned bounded inbox for runtime Wi-Fi packets, RPQ,
other C2H and unrelated ACKs; keep matched ACK and BTC policy routing intact.
Retain bytes and descriptor metadata before scratch reuse, expose ordered
borrowed records, and make overflow terminal with the offending scratch
record still inspectable. The existing no-inbox constructor remains a
pre-data startup diagnostic only. Runtime activation must require the inbox.
Refactor the private TX engine to ingest retained records and advance DMA
without reading RX rings; keep native TX and receive-only guards unchanged.
Validate command-wait interleaving, copy ownership, capacity, terminal failure
and source-preserving TX completion on the owning branch after publication.


### Owned initial packet-mode prerequisite

The [private data-mode continuation](../hwdocs/rtl8852be-data-mode.md) consumes
the actual startup Session and configured station, binds command/RX/TX identities
and checks scheduler/DMA/radio state before exposing its internal packet API.
The original startup ReceivePermit remains strict. One RX dispatcher handles
ACKs, packet completion and software filtering during command waits; ordinary
Wi-Fi pressure is counted/dropped instead of terminating the connection.
Thirteen cold continuation cases and all thirteen affected host harnesses
passed on own corrected source `974088b1` after raw `33fa88bf` publication
(2026-10-16; counts, unchanged private ignore and first failed fixture run
recorded in the linked notes). Continuous BTC event
processing, H2C sequence reuse, native service exposure and physical under-load
acceptance remain open. This slice does not establish a working Wi-Fi endpoint.
