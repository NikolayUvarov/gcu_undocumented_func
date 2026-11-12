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
# Every tests/*_host.rs, but those built on a line of their own (flags, a crate, an order): a new test needs no line
# here, so tracks that add tests do not meet in this file (000-KRN-0238).
OWN_LINES=" blockstore hostname index journal listen network_budget nts regdb rtw89_service rtw89_station_filter tls_exporter tts vector voice "
for file in tests/*_host.rs; do
    test=$(basename "$file" _host.rs)
    [[ $OWN_LINES == *" $test "* ]] && continue
    rustc --edition=2021 --test "tests/${test}_host.rs" -o "$OUT/$test-tests"
    "$OUT/$test-tests"
done
# The clipboard service's state: its parts, who is answered, the generation (000-APP-0032).
rustc --edition=2021 --test tests/clipboard_host.rs -o "$OUT/clipboard-tests"
"$OUT/clipboard-tests"
# The WPA2-PSK library uses RustCrypto crates, so Cargo builds its tests and the station's (550-NET-0016, 0017).
cargo test --quiet --manifest-path wpa/Cargo.toml --target-dir "$OUT/wpa-target"
cargo test --quiet --manifest-path dot11/Cargo.toml --target-dir "$OUT/wpa-target" # the station (550-NET-0017)
cargo test --quiet --manifest-path apppkg/Cargo.toml --target-dir "$OUT/wpa-target" # signed application packages (750-UPD-0019)
cargo test --quiet --manifest-path passlock/Cargo.toml --target-dir "$OUT/wpa-target" # the device key under a passphrase, Argon2id against RFC 9106 (308-NET-0055)
cargo test --quiet --manifest-path nnpkg/Cargo.toml --target-dir "$OUT/nnpkg-target" # nnd's package checks and clients on NRL's pinned model (262-APP-0112)
cargo test --quiet --manifest-path tests/network_budget/Cargo.toml --target-dir "$OUT/network-budget-target" # the deadlines (550-NET-0031) and a listening port's backlog (110-NET-0052, tests/listen_host.rs)
cargo test --quiet --manifest-path tests/tls_exporter/Cargo.toml --target-dir "$OUT/tls-exporter-target" # the TLS service's exporter against OpenSSL's (111-NET-0073)
cargo test --quiet --manifest-path tests/nts/Cargo.toml --target-dir "$OUT/nts-target" # timed's NTS: AES-SIV against RFC 5297, NTS-KE, the fields, the parsers fuzzed (111-NET-0075)
cargo test --quiet --manifest-path linkproto/Cargo.toml --target-dir "$OUT/linkproto-target" # the link protocol (800-LNK-0001)
cargo test --quiet --manifest-path tools/mindlink/Cargo.toml --target-dir "$OUT/linkproto-target" # its Linux end and effector485, over a pty and TCP (800-LNK-0004, 0011)
cargo test --quiet --manifest-path tools/httpd/Cargo.toml --target-dir "$OUT/httpd-target" # the release server: the layout, ranges, limits, the log (110-NET-0066)
cargo test --quiet --features test-hooks --manifest-path tools/httpd/Cargo.toml --target-dir "$OUT/httpd-target" --lib --test hooks # its test hooks (110-NET-0068)
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
# The vector index's tests score every vector of journals of hundreds of embeddings for each query, also by the exact
# scan: optimized, its checks of overflow and assertions kept, 1 s rather than 13 (312-STO-0047).
rustc --edition=2021 -O -C debug-assertions=on --test tests/vector_host.rs -o "$OUT/vector-tests"
"$OUT/vector-tests"
# A model as one object of the store (251-STO-0015), on a line of its own so that it meets no shared line.
rustc --edition=2021 --test tests/model_object_host.rs -o "$OUT/model_object-tests"
"$OUT/model_object-tests"
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
python3 tests/qemu_trial_test.py # a trial boot's confirmation read from the console's log (000-KRN-0210)
python3 tests/qemu_stuck_test.py # a wait that ran out keeps the CPUs' registers (000-KRN-0214)
python3 tests/app_package_test.py
python3 tests/test_usb_writer.py
python3 tests/boot_switch_test.py
python3 tests/mac_startup_disk_test.py # an Intel Mac's startup disk from macOS Recovery (800-LNK-0029)
python3 tests/build_and_write_args_test.py # the writer's --mode= refusals (751-LNK-0030)
bash tests/build_and_write_keep_test.sh # a damaged old disk never erases what the writer kept (800-LNK-0036)
python3 tests/gate_test.py
python3 tests/build_jobs_test.py
python3 tests/mirror_test.py
python3 tests/usb_image_test.py
python3 tests/board_image_test.py # an Allwinner board's card: the partitions moved past U-Boot (100004-MOB-0001)
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
python3 tests/regdb_test.py
rustc --edition=2021 --test tests/regdb_host.rs -o "$OUT/regdb-tests"
"$OUT/regdb-tests"
rustc --edition=2021 --test tests/rtw89_station_filter_host.rs -o "$OUT/rtw89-station-filter-tests"
"$OUT/rtw89-station-filter-tests"
rustc --edition=2021 --cfg rtw89_service --test tests/rtw89_service_host.rs -o "$OUT/rtw89-service-tests"
"$OUT/rtw89-service-tests"
rustc --edition=2021 --cfg 'feature="alloc"' --test tests/hostname_host.rs -o "$OUT/hostname-tests" # the machine's name (000-KRN-0180)
"$OUT/hostname-tests"
