#!/usr/bin/env bash
# The host tests of CI's build job and of scripts/ci_local.sh: each compile and each test fails the run (175-KRN-0046).
# MIND_TEST_OUT: where the test binaries go (default $TMPDIR, else /tmp).
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
OUT=${MIND_TEST_OUT:-${TMPDIR:-/tmp}}
rustc --edition=2021 --test tests/runtime.rs -o "$OUT/runtime-tests"
"$OUT/runtime-tests"
rustc --edition=2021 --test tests/tts_host.rs -o "$OUT/tts-tests"
"$OUT/tts-tests"
for test in heap keys tui viewer idl rtc sysmon monitor fm block fat edit logd search bmp netring window wm clock virtio_input hid battery aml gpio pins wifi path video uvc iso_ring line beep console say jpeg script diag cid blockstore dag checkpoint boot_slots http tpm fbank nn russian syntax shell image bench vfs_zone release models idl_fuzz updater json rtl8168 rtw89_firmware rtw89_power rtw89_fwcmd rtw89_mac rtw89_pcie rtw89_download rtw89_runtime_mac rtw89_rx rtw89_phy rtw89_startup rtw89_pipeline realtek_probe pci_control boot_abi io; do
    rustc --edition=2021 --test "tests/${test}_host.rs" -o "$OUT/$test-tests"
    "$OUT/$test-tests"
done
# The WPA2-PSK library uses RustCrypto crates, so Cargo builds its tests and the station's (550-NET-0016, 0017).
cargo test --quiet --manifest-path wpa/Cargo.toml --target-dir "$OUT/wpa-target"
cargo test --quiet --manifest-path dot11/Cargo.toml --target-dir "$OUT/wpa-target" # the station (550-NET-0017)
cargo test --quiet --manifest-path tests/network_budget/Cargo.toml --target-dir "$OUT/network-budget-target"
# The SSH client against OpenSSH's server, which its tests start on 127.0.0.1 (351-NET-0026): needs openssh-server.
cargo test --quiet --manifest-path ssh/Cargo.toml --target-dir "$OUT/ssh-target"
# The voice tests synthesize and recognize ~1000 phrases and time recognition: optimized, as the system is.
rustc --edition=2021 -O --test tests/voice_host.rs -o "$OUT/voice-tests"
"$OUT/voice-tests"
python3 tests/idl_test.py
python3 tests/font_test.py
python3 tests/manifest_test.py
python3 tests/release_test.py
python3 tests/test_usb_writer.py
python3 tests/gate_test.py
python3 tests/mirror_test.py
python3 tests/usb_image_test.py
python3 tests/media_test.py
python3 tests/effector_server_test.py
python3 tests/rtl8168_probe_test.py
python3 tests/rtl8852be_fetch_test.py
python3 tests/rtl8852be_probe_test.py

python3 tests/rtw89_phy_tables_test.py
