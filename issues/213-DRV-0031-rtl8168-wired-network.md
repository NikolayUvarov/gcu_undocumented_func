# 213-DRV-0031 — A wired RTL8168 path on the Ryzen for ordinary use and Effector

**Type:** driver · **Owner:** ASTRA auditor-fixer, for `DRV`, with `KRN` and `NET` integration · **Priority:** P1 · **Status:** in progress (2026-11-26: on the Ryzen the wired link carries DHCP, ping outward and 30 minutes of transfers, and comes back after the driver's restart; left: the cable taken out and put back, an HTTPS request, and the echo from the LAN) · **Blocked by:** the maintainer's hands on the cable; `NET`'s word on the echo · **Main task:** [213](213-amd-ryzen-pc.md), [550](550-network-on-real-hardware.md) · **Roadmap:** tracks H, A, D · **Constitution:** MC-2.3, MC-3.3, MC-6.1, MC-12.1, MC-12.3, Appendix B.6

## Problem

Ryzen build e6730d7a boots the network stack without a driver for its installed `10ec:8168` Ethernet controller. Ping and an Effector agent cannot work over a NIC the operating system does not drive. The separate RTL8852BE Wi-Fi device is not covered by this task. The timeout/fairness changes in [550-NET-0031](550-NET-0031-bounded-driver-calls-and-ingress.md) do not add hardware support.

The maintainer requested both Wi-Fi and Ethernet, with Wi-Fi first (2026-10-15). This wired task can proceed in parallel; it is not a replacement for the Wi-Fi task.

## Plan

1. **Review the identified variant.** Task 213-DRV-0030 observed XID `0x4c0`, RTL8168g/8111g, on the actual Ryzen; the pinned source mapping is in [hwdocs/rtl8168.md](../hwdocs/rtl8168.md#ryzen-observation-2026-10-15). Match only reviewed variants; report an unknown XID and leave the device stopped. Derive this variant's reset, PHY negotiation and power-management sequence from primary specifications or reviewed, licence-compatible sources; record those sources and any reused code in `THIRD_PARTY.md`. No speculative PHY writes.
2. **Provide a ring-3 driver.** A proposed `rtl8168` service receives its own MMIO BAR, DMA memory, interrupt and service endpoint. Read and validate the MAC; bound every reset, PHY and descriptor wait. Set up receive/transmit descriptor rings with ownership, length and wrap checks; use memory barriers at ownership changes; reclaim a descriptor only after completion. Start with 1500-byte MTU, software checksums and no segmentation offload. Count receive errors, queue overflow, transmit failures and interrupt activity. Keep the service responsive when link negotiation or transmission fails.
3. **Provide boot grants as a kernel-track task.** `init` finds class `02:00:00` and exact PCI ID `0x8168_10ec`, takes the observed device's current BAR2 through the platform capability path, lends bounded DMA memory below 4 GiB, and grants MSI through the existing `PLATFORM_DEVICE_MSI`. It records the device for normal quiesce/restart handling. The reported legacy line `255` is not a usable fallback. Do not grant arbitrary PCI configuration access or hard-code the observed BAR address. If the verified variant requires an operation current capabilities cannot express, request an explicit KRN interface task.
4. **Use the existing card protocol.** Serve `idl/net.wit` 1.3: info/link, frame send/receive, deferred wait, counters, read-only badges, and zero offloads. Implement the existing shared frame ring or explicitly reject attach until it is supported. Return bounded, documented errors on absent/down/busy hardware. Add the service to the image and startup registry through KRN's task.
5. **Connect it to the stack.** NET/KRN agree which wired card slot carries RTL8168, preserving existing VirtIO cards and WLAN. Ensure default-route selection and DHCP/link transitions use the same code as other wired cards. A missing or disconnected NIC must not stall other cards or socket requests. No service ABI change is needed solely to introduce this driver.
6. **Handle firmware explicitly.** Determine from the identified variant whether an external firmware file is needed. If its terms forbid redistribution, use the project's named-source fetch, SHA-256 validation and post-image-write `data/firmware/` installation. Never commit proprietary payloads or put them in an image; a missing required file must be reported.
7. **Validate before claiming support.** Host tests cover descriptor serialization, boundaries, ownership/reclaim, malformed receive lengths and bounded wait/recovery. QEMU regression checks protect existing network services; an RTL8139 emulation is not evidence for RTL8168. Then test the exact Ryzen, recording the build, XID, interface settings, cable/switch and results.

## Acceptance criteria

- The installed variant is explicitly identified and supported; unsupported variants fail clearly.
- On this Ryzen, a connected Ethernet link obtains an address and route through DHCP; bidirectional ping and an authenticated HTTPS request through the existing TLS/policy services work.
- Record at least 30 minutes of ping alongside sustained TCP transfer and the maintainer's previously slow tests: packet counts/loss, latency, throughput, NIC counters and kernel/service failures. Any loss or pause is explained and bounded rather than hidden in an aggregate success message.
- Unplug/replug, link negotiation failure and driver restart leave the stack responsive; a restored link reacquires its lease and new flows work. No old DMA buffers are reused while the device still owns them.
- No unresolved transmit/receive timeout, DMA error or service hang remains in the validation run. Measurements and known limits are recorded per machine/build in `docs/profile`.
- Effector's connectivity task may depend on this result, but completing the driver does not itself establish a working Effector agent or server session.

## Related

[213-DRV-0030](../issues-done/213-DRV-0030-rtl8168-hardware-identification.done), [213](213-amd-ryzen-pc.md), [550](550-network-on-real-hardware.md), [550-DRV-0005](550-DRV-0005-usb-ethernet.md), [550-NET-0031](550-NET-0031-bounded-driver-calls-and-ingress.md), [750](750-effector-agent.md), [requests-DRV.md](requests-DRV.md), `idl/net.wit`, `init/src/main.rs`.

## Progress (2026-10-15)

The independent `rtl8168` no_std library now implements a read-only TxConfig/XID decoder and coherent x86-64 descriptor rings: checked DMA layout below 4 GiB, bounded single-frame send/receive/reclaim, TX padding and full-ring handling, volatile ownership with compiler/hardware fences, RX validation/FCS removal and wrap handling. Its unsafe contract requires exclusive stable coherent memory and a quiesced device before reuse. This first stage contained no executable, startup registration or PHY/reset writes; the next stage below adds an experimental service without claiming a working Ethernet path.

Eight host regressions in `tests/rtl8168_host.rs` pass, including independent raw descriptor values, full/wrapped queues, ownership retention on short output, malformed input and unknown/inaccessible identity. The crate checks for `x86_64-unknown-none`. Primary sources, exact layout, safety conditions and remaining hardware-dependent work are recorded in [hwdocs/rtl8168.md](../hwdocs/rtl8168.md). That descriptor stage copied no Linux or BSD driver implementation; the separate controller adaptation below retains its BSD licence.

Task 0030 now supplies actual Linux hardware evidence from `2026-10-15T18:29:02Z`: TxConfig `0x4f000f80`, XID `0x4c0`, with verified `10ec:8168` revision `0c`, PHY ID `0x001cc800` and the driver's reported `rtl8168g-2_0.0.1 02/06/13` firmware version. The source-selected initialization and firmware filename are documented in the hardware notes; this identifies the implementation target without claiming support. Carrier was absent during collection. The initial identification left device startup and integration open; the bounded baseline below advances them, while firmware need, boot grants and connected-link/load/recovery acceptance remain unvalidated. Identification no longer blocks implementation review.

### Experimental controller and executable

The `rtl8168` executable now implements the existing card service with zero offloads, bounded RX/TX work, read-only badge enforcement and explicit unsupported shared-ring rejection. Its controller restricts startup to the actual XID/PHY pair, uses a single 1500 ms startup budget, and separates reset/PHY negotiation, ring programming and engine enable. The PCI index arrives as a decimal process argument; initial grant must enable memory decoding only. Ring programming precedes a checked `platform::resume(index)`, and engines remain disabled until that succeeds. Kernel integration belongs to `213-KRN-0114-realtek-driver-startup.md`, including boot image, exact BAR2/DMA/MSI grants, card slot and restart quiescence. No system-call ABI changes are made by this driver task.

Runtime faults and a 5 s TX-completion watchdog stop new work, request platform quiescence and retain DMA even if that request fails. Normal supervision must verify quiescence before memory reuse/restart. The service does not claim automatic recovery from a hardware hang.

The startup register baseline is adapted from FreeBSD commit `16a79708d089423ec0c3d32f25e7ca97ae58e97b` under BSD-4-Clause, with original notices, exact source hashes and the required acknowledgement in `LICENSES/`, `THIRD_PARTY.md` and the hardware notes. This product includes software developed by Bill Paul. No Linux GPL implementation or firmware payload is imported. The baseline does not apply Linux's PHY patch or vendor tuning; its adequacy on a cold Ryzen boot remains a concrete hardware-validation question, not a guarantee.

Own-branch release build passes. After own-branch implementation commit `e89fb569` was published through fast-test merge `91059a37`, all sixteen focused host regressions passed: eight new controller regressions and eight existing descriptor/identity regressions. The staged DMA/master-enable delta received an independent read-only review with no blocking defect; its verified-quiesce claim requires KRN-0114's checked PCI readback. Further pinned FreeBSD model decoding identifies PHY `001cc800` as RTL8251, which that driver explicitly excludes from legacy DSP programming; this supports the selected baseline without asserting equivalence to Linux's tuning. The hardware notes contain the first-boot evidence checklist. The task stays open until connected-link, DHCP, authenticated HTTPS, load, unplug/replug and restart criteria are measured on the actual Ryzen. QEMU cannot emulate this RTL8168G variant.

### Deadline audit follow-up (2026-10-16)

The pre-hardware review confirmed that the actual controller wait accepted a
successful MMIO observation before checking its 50 ms PHYAR / 100 ms reset
deadline. The separate PHY-reset loop accepted clear BMCR before its 500 ms
check, and a final read could also cross the global 1500 ms startup limit.
Keep all limits and finite iteration ceilings; validate monotonic time and
both local/global budgets before and after the real production read. Add
`prepare()` regressions for a boundary success, exact/late cutoff, oversleep,
clock regression and no subsequent DMA/negotiation publication. Record the
finding in `issues-audit` and actual post-publication host evidence separately.
This is a deterministic controller bug, not a measured explanation of the
Ryzen's currently unvalidated Ethernet connectivity or PHY tuning needs.


The [H15 correction](../issues-audit/2026-10-16-rtl8168-deadline-audit.md)
was committed as `f5ee34b08374c3a8a6267d95817cdc7cce039c44`, built on the own
branch and published through raw merge
`bc802db7a0ce06aae8b93da538964b57a10fb3d7`. The own-branch RTL8168 harness then
passed 22/22 host tests (no failures, ignores or filtering), including six
new production-path cutoff regressions. No physical NIC mutation or traffic
test was performed, and the hardware acceptance criteria remain open.

## On the Ryzen with a cable (2026-11-26)

The maintainer plugged a cable in during boot 103 (release 202611260804) and left it through boot 104 (202611260856). Both are `main` 6230e1fdb with DRV's pending requests; `rtl8168` and `netstack` are `main`'s. It holds for that machine, that switch and those releases (MC-12.1). The raw lines are kept by the owner session (`logs-2026-11-26/`: `wired30.txt`, `ping30.txt`, `benchsrv.log`).

**Against the criteria:**
1. **The variant:** `[RTL8168] XID=4C0 PHY=001CC800 LINK=DOWN MODE=MSI RINGS=256; EXPERIMENTAL BSD BASELINE` at each start, as before.
2. **Address and route:** `[NETSTACK] CARD 0 LINK UP, WAITING FOR DHCP`, then `DHCP 192.168.2.241/24 GATEWAY 192.168.2.1 DNS 8.8.8.8 ON CARD 0`; `ip` says `CARD 0 … WIRED DEFAULT` beside the Wi-Fi's card 2.
   - **Ping outward:** the gateway 5 of 5 (10.5 to 12.9 ms on an idle machine), 8.8.8.8 3 of 3, the Effector server 3 of 3.
   - **Ping inward is not answered:** an echo request from the LAN to 192.168.2.241 gets no reply, and none to the Wi-Fi's address either, while ARP resolves both. So it is the stack's or the policy's, not this card's: asked of `NET` (`KRN/2026-11-26T0903Z-DRV-the-ryzens-window-the-lines-you-wait-for-from-boots-102-to-104.md` on `internal-dev-messages`). "Bidirectional ping" stays open on that answer.
   - **An HTTPS request is not made yet:** `get` to the server is not in its grant and asks the user at the machine's screen.
3. **Thirty minutes of ping with transfers** (09:26 to 09:56 UTC; `ping -c 1800` to the gateway typed at the console, `netbench` to a server on the LAN over and over by the cable's `run`):
   - **59 transfers:** 64 MiB downward each at 3539 to 5122 KiB/s, the median 4805. Upward each stops at 33 MB with `NETBENCH SEND FAILED: Denied`: that is the bench's own policy line (100 MB a flow), not the card; the server saw them at 1803 to 2776 KiB/s, the median 2111.
   - **Ping:** 1800 sent, 1793 answered, 7 timed out (0.4 %). The least 331 us, the median 746 us; a tenth above 179 ms, a hundredth above 510 ms, the longest 823 ms; 236 above 100 ms.
   - **What the pauses are, as the log reads:** every minute `[NETSTACK] CARD 0 IN THE LAST MINUTE: 53547 to 67091 FRAMES TAKEN, 0 to 4 REFUSED OR UNANSWERED, about 337000 VISITS PASSED OVER AS ITS 2000 MS OF DRIVER TIME IN 5 S WERE SPENT`. So the stack's budget of driver time for the card (550-NET-0031) is spent throughout the transfer, and a ping waits behind it. That bounds the throughput too. Whether the budget fits a wired gigabit card is `NET`'s; the driver reported no timeout, no DMA error and no fault in the run.
   - **Counters after it:** `SENT=1912150 RECEIVED=3186068`.
   - **Short transfers on the idle machine:** 4 MiB downward at 6427 KiB/s, upward at 2731 KiB/s, 500 of 500 UDP round trips at 355 a second.
4. **The driver restarted once** (`kill`, uptime 3457.4 s): `[INIT] rtl8168 RESTARTED PID=115`, `LINK=DOWN`, `[NETSTACK] DEFAULT ROUTE: CARD 2`, 3 s later `CARD 0 LINK UP, ASKING DHCP WHETHER 192.168.2.241 IS STILL ITS ADDRESS`, `DEFAULT ROUTE: CARD 0`, `CARD 0 BACK ON ITS NETWORK AFTER 13054 MS: ADDRESS AND 0 CONNECTIONS KEPT`. A transfer after it: 4 MiB downward at 6464 KiB/s, upward at 2108 KiB/s, 500 of 500 round trips.
   - **Not done:** the cable taken out and put back (the maintainer's hands), and a link that fails to negotiate.
5. **No timeout, DMA error or hang** of the driver in these runs. `docs/profile` is not changed yet: it gets the numbers when the task closes.

**Seen beside it:** with the cable plugged in after the start, the Effector agent's session stayed on the Wi-Fi (card 0 had sent 4 frames after 25 minutes of boot 104) though the wire was the default route. Told to `NET` in the same message.
