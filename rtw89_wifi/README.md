# RTL8852BE firmware diagnostic and radio prerequisites

This native service verifies the separately supplied private firmware file,
reads board data under the MEM-only BAR2 grant supplied by KRN-0114, and
attempts one bounded command-DMA/WCPU download for the supported cut-1/RFE-1
board/security profile. It queries actual firmware PHY capabilities before
runtime MAC preparation and receive-only DMA activation, then consumes the
firmware token in one owned Session for normal H2C, actual BTC initialization,
BB/RF tables, initial DM, NCTL/RCK/DACK/RXDCK, board trims and the report tail. A missing/malformed
response stops the attempt; zero NSS in a successful reply is logged separately
from the documented chip-default fallback. Independent stop, kernel mastering-disable checks
and power-off follow the attempt; the coherent DMA grant stays retained.
Channel IQK/TSSI/DPK, actual channel/power policy and association remain
unimplemented in this native path; it registers no radio and provides no network link.

The library also contains the independently testable firmware parser,
register/OTP access state machine, MAC download-memory/PCIe preparation and
channel-12 PCIe command queue. The native diagnostic now joins the initial
PHY stages over actual session ownership; its observation result cannot resume
the ended command engine or authorize packet transmission. H2C tracks at most
eight results and 256 non-reused sequences in a fresh firmware session;
timeout or uncertain publication poisons it without automatic retransmission.
Initial DACK/RXDCK use the owned BTC antenna/grant/TDMA policy and bounded
START/STOP arbitration; a firmware ACK alone never grants calibration.
[Hardware notes](../hwdocs/rtl8852be.md)
record the immutable BSD sources, exact bounds, error behavior and validation
limits. [DRV-0032](../issues/213-DRV-0032-rtl8852be-wifi.md) remains open until
actual association and traffic under load meet its acceptance criteria.

Build from this directory with `cargo build --release --locked`. The focused
host fixtures are `tests/rtw89_power_host.rs`, `tests/rtw89_mac_host.rs`,
`tests/rtw89_pcie_host.rs`, `tests/rtw89_fwcmd_host.rs`,
`tests/rtw89_h2c_host.rs` and `tests/rtw89_btc_host.rs`;
the command-queue fixture uses an x86-64 host and simulated coherent DMA.
`scripts/host_tests.sh` runs them as part of the host group. Build and test the agent's own branch only, following
the commit/build/fast-test-publication/test order in `AGENTS.md`.
