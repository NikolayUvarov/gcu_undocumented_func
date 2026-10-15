# RTL8852BE firmware diagnostic and radio prerequisites

This native service verifies the separately supplied private firmware file,
reads board data under the MEM-only BAR2 grant supplied by KRN-0114, and
attempts one bounded command-DMA/WCPU download for the supported cut-1/RFE-1
board/security profile. Independent stop, kernel mastering-disable checks
and power-off follow the attempt; the coherent DMA grant stays retained.
It does not calibrate RF, register a radio or provide a network link.

The library also contains the independently testable firmware parser,
register/OTP access state machine, MAC download-memory/PCIe preparation and
channel-12 PCIe command queue. Additional runtime-MAC, RX/C2H and normal-H2C
libraries are not activated by the native diagnostic. H2C tracks at most
eight results and 256 non-reused sequences in a fresh firmware session;
timeout or uncertain publication poisons it without automatic retransmission.
The BT RFK helper returns only a bounded read-only observation, never a
calibration grant. Actual BTC antenna/grant/TDMA policy remains required.
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
