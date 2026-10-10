#!/usr/bin/env bash
# The host tests of CI's build job and of scripts/ci_local.sh: each compile and each test fails the run (175-KRN-0046).
# MIND_TEST_OUT: where the test binaries go (default /tmp).
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
OUT=${MIND_TEST_OUT:-/tmp}
rustc --edition=2021 --test tests/runtime.rs -o "$OUT/runtime-tests"
"$OUT/runtime-tests"
rustc --edition=2021 --test tests/tts_host.rs -o "$OUT/tts-tests"
"$OUT/tts-tests"
for test in heap keys tui viewer idl rtc sysmon monitor fm block fat edit logd search bmp netring window wm clock virtio_input hid aml gpio pins video uvc iso_ring line beep console say jpeg script cid blockstore dag checkpoint boot_slots http tpm bench vfs_zone release models idl_fuzz; do
    rustc --edition=2021 --test "tests/${test}_host.rs" -o "$OUT/$test-tests"
    "$OUT/$test-tests"
done
# The voice tests synthesize and recognize ~1000 phrases and time recognition: optimized, as the system is.
rustc --edition=2021 -O --test tests/voice_host.rs -o "$OUT/voice-tests"
"$OUT/voice-tests"
python3 tests/idl_test.py
python3 tests/font_test.py
python3 tests/manifest_test.py
python3 tests/release_test.py
python3 tests/test_usb_writer.py
python3 tests/gate_test.py
python3 tests/usb_image_test.py
