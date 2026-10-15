# RTL8852BE initial board access

This native service reads board data under the MEM-only BAR2 grant supplied
by KRN-0114, then powers the MAC down. It supports the observed digital cut 1
and RFE 1. It does not download firmware, start DMA, calibrate RF, register a
radio or provide a network link.

The library also contains the independently testable firmware parser and
register/OTP access state machine. [Hardware notes](../hwdocs/rtl8852be.md)
record the immutable BSD sources, exact bounds, error behavior and validation
limits. [DRV-0032](../issues/213-DRV-0032-rtl8852be-wifi.md) remains open until
actual association and traffic under load meet its acceptance criteria.

Build from this directory with `cargo build --release --locked`. The focused
host fixture is `tests/rtw89_power_host.rs`; `scripts/host_tests.sh` runs it as
part of the host group. Build and test the agent's own branch only, following
the commit/build/fast-test-publication/test order in `AGENTS.md`.
