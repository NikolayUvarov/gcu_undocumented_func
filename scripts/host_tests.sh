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
for test in heap keys tui viewer idl record rtc sysmon monitor fm block fat edit logd search bmp netring window wm clock virtio_input hid battery aml gpio pins wifi path video uvc iso_ring line beep console say jpeg script diag cid netpolicy dag tree worktree car assertion storefs fs_routing checkpoint boot_slots http tpm fbank nn russian syntax shell image bench vfs_zone release models idl_fuzz updater json rtl8168 rtw88_tables rtw88_driver rtw89_firmware rtw89_power rtw89_fwcmd rtw89_mac rtw89_pcie rtw89_download rtw89_phy rtw89_iqk rtw89_calibration rtw89_channel_policy rtw89_channel_power rtw89_startup rtw89_pipeline rtw89_pipeline_mac rtw89_initial_phy rtw89_channel_rfk rtw89_station rtw89_calibration_policy rtw89_runtime_mac rtw89_phycap rtw89_rx rtw89_frames rtw89_data_mode rtw89_native_dma rtw89_connection rtw89_tx rtw89_post_mac rtw89_h2c rtw89_btc rtw89_btc_init rtw89_btc_reports rtw89_btc_codec rtw89_btc_control rtw89_btc_session rtw89_dpk realtek_probe fch_watchdog pci_control boot_abi io bcm_txwait bcm_level serial usb_hub effector_session effector_policy effector_json effector_wire effector_config effector_journal effector_gateway effector_png effector_telemetry effector_bench effector_update zip wifijoin wlan_auto remote_marks services svc bt_hci edid sse multipart http_coding link_names secrets get net_questions boot_path; do
    rustc --edition=2021 --test "tests/${test}_host.rs" -o "$OUT/$test-tests"
    "$OUT/$test-tests"
done
# The WPA2-PSK library uses RustCrypto crates, so Cargo builds its tests and the station's (550-NET-0016, 0017).
cargo test --quiet --manifest-path wpa/Cargo.toml --target-dir "$OUT/wpa-target"
cargo test --quiet --manifest-path dot11/Cargo.toml --target-dir "$OUT/wpa-target" # the station (550-NET-0017)
cargo test --quiet --manifest-path nnpkg/Cargo.toml --target-dir "$OUT/nnpkg-target" # nnd's package checks and clients on NRL's pinned model (262-APP-0112)
cargo test --quiet --manifest-path tests/network_budget/Cargo.toml --target-dir "$OUT/network-budget-target"
cargo test --quiet --manifest-path apppkg/Cargo.toml --target-dir "$OUT/wpa-target" # signed application packages (750-UPD-0019)
cargo test --quiet --manifest-path linkproto/Cargo.toml --target-dir "$OUT/linkproto-target" # the link protocol (800-LNK-0001)
cargo test --quiet --manifest-path tools/mindlink/Cargo.toml --target-dir "$OUT/linkproto-target" # its Linux end and effector485, over a pty and TCP (800-LNK-0004, 0011)
# The multi-device firmware's logic: frames between the boards, credits, control commands, the LED (800-LNK-0005).
cc -std=c11 -Wall -Wextra -Werror -O1 -o "$OUT/rp2-link-tests" devices/rp2-link/tests/host_test.c devices/rp2-link/src/bridge.c devices/rp2-link/src/control.c devices/rp2-link/src/led.c
"$OUT/rp2-link-tests"
# The SSH client against OpenSSH's server, which its tests start on 127.0.0.1 (351-NET-0026): needs openssh-server.
cargo test --quiet --manifest-path ssh/Cargo.toml --target-dir "$OUT/ssh-target"
# The store's tests seal every record and time 100 000 puts: optimized, its checks of overflow and assertions kept (308-STO-0021).
rustc --edition=2021 -O -C debug-assertions=on --test tests/blockstore_host.rs -o "$OUT/blockstore-tests"
"$OUT/blockstore-tests"
# The journal's tests append 1000 entries, each published after the store's walk of the whole chain: optimized, its
# checks of overflow and assertions kept, they take seconds, not a minute (312-STO-0034).
rustc --edition=2021 -O -C debug-assertions=on --test tests/journal_host.rs -o "$OUT/journal-tests"
"$OUT/journal-tests"
# The index's tests search every word of journals of hundreds of entries, each answer checked against the journal and
# each word scanned too: optimized, its checks of overflow and assertions kept, 4 s rather than 44 (312-STO-0035).
rustc --edition=2021 -O -C debug-assertions=on --test tests/index_host.rs -o "$OUT/index-tests"
"$OUT/index-tests"
# The voice tests synthesize and recognize ~1000 phrases and time recognition: optimized, as the system is.
rustc --edition=2021 -O --test tests/voice_host.rs -o "$OUT/voice-tests"
"$OUT/voice-tests"
python3 tests/idl_test.py
python3 tests/land_test.py # the landing queue's requests, plan and candidate (AGENTS.md, section 4)
python3 tests/link_update_test.py # an update through the link: what goes over the cable, the log followed (351-KRN-0141)
python3 tests/kernel_budget_test.py
python3 scripts/kernel_budget.py # the kernel's size against kernel/size-budget.json (850-KRN-0145)
python3 tests/font_test.py
python3 tests/manifest_test.py
python3 tests/release_test.py
python3 tests/app_package_test.py
python3 tests/test_usb_writer.py
python3 tests/boot_switch_test.py
python3 tests/mac_startup_disk_test.py # an Intel Mac's startup disk from macOS Recovery (800-LNK-0029)
python3 tests/build_and_write_args_test.py # the writer's --mode= refusals (751-LNK-0030)
python3 tests/gate_test.py
python3 tests/build_jobs_test.py
python3 tests/mirror_test.py
python3 tests/usb_image_test.py
python3 tests/media_test.py
python3 tests/effector_server_test.py
python3 tests/rtl8168_probe_test.py
python3 tests/rtl8852be_fetch_test.py
python3 tests/rtl8852be_probe_test.py

python3 tests/rtw89_phy_tables_test.py
python3 tests/rtw89_dack_tables_test.py

python3 tests/rtw89_power_tables_test.py

python3 tests/rtw89_iqk_tables_test.py
python3 tests/rtw89_dpk_tables_test.py
python3 tests/rtw89_tssi_tables_test.py
python3 tests/rtw89_regdb_test.py
rustc --edition=2021 --test tests/rtw89_regulatory_host.rs -o "$OUT/rtw89-regulatory-tests"
"$OUT/rtw89-regulatory-tests"
rustc --edition=2021 --test tests/rtw89_station_filter_host.rs -o "$OUT/rtw89-station-filter-tests"
"$OUT/rtw89-station-filter-tests"
rustc --edition=2021 --cfg rtw89_service --test tests/rtw89_service_host.rs -o "$OUT/rtw89-service-tests"
"$OUT/rtw89-service-tests"
