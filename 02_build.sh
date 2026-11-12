#!/bin/bash
set -o pipefail

BUILD_SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# The branch and commit built, for init's first line in the boot log (211-KRN-0044); "+CHANGES" when files differ.
MIND_BUILD_BRANCH="$(git -C "$BUILD_SCRIPT_DIR" rev-parse --abbrev-ref HEAD 2>/dev/null || echo unknown)"
MIND_BUILD_COMMIT="$(git -C "$BUILD_SCRIPT_DIR" rev-parse --short=12 HEAD 2>/dev/null || echo unknown)"
[[ -n "$(git -C "$BUILD_SCRIPT_DIR" status --porcelain --untracked-files=no 2>/dev/null)" ]] && MIND_BUILD_COMMIT+="+CHANGES"
# The commit's time, in seconds since 1970: timed never sets the clock before it (111-NET-0057); the same for every build of
# the commit, so the build stays reproducible.
MIND_SOURCE_TIME="$(git -C "$BUILD_SCRIPT_DIR" log -1 --format=%ct 2>/dev/null || echo 0)"
export MIND_BUILD_BRANCH MIND_BUILD_COMMIT MIND_SOURCE_TIME
# ARCH=aarch64 builds the aarch64 image instead (aarch64_root/, issue 204); arguments go to scripts/build_aarch64.sh.
case "${ARCH:-x86_64}" in
    x86_64) ;;
    aarch64) source "$HOME/.cargo/env" 2>/dev/null || true; exec "$BUILD_SCRIPT_DIR/scripts/build_aarch64.sh" "$@" ;;
    *) echo "ARCH must be x86_64 or aarch64, not $ARCH" >&2; exit 2 ;;
esac
LOG_DIR="$BUILD_SCRIPT_DIR/code_handoff"
LOG_FILE="$LOG_DIR/build.log"

mkdir -p "$LOG_DIR"
: > "$LOG_FILE"
exec > >(tee -a "$LOG_FILE") 2>&1

source "$HOME/.cargo/env" 2>/dev/null || true
export PATH="$HOME/.cargo/bin:$PATH"

START_TS=$(date +%s)
STEP="initialization"
FAILED=0

fail() {
    FAILED=1
    echo
    echo "!!! ERROR at step: $STEP"
    finish
}

finish() {
    local elapsed=$(( $(date +%s) - START_TS ))
    echo
    echo "=========================================================="
    if [ "$FAILED" -eq 0 ]; then
        echo "RESULT: SUCCESS"
        echo "Build time: ${elapsed} s"
    else
        echo "RESULT: FAILURE"
        echo "Failed step: $STEP"
    fi
    echo "=========================================================="
    exec 1>&- 2>&-
    wait
    exit "$FAILED"
}

trap fail ERR
set -e

USER_CRATES=(
    "kernel:kernel:kernel.elf"
    "init:init:init.elf"
    "shell:shell:shell.elf"
    "app:app:app.elf"
    "app2:app2:app2.elf"
    "clock:clock:clock.elf"
    "dzen-clock:dzen-clock:dzen-clock.elf"
    "ping:ping:ping.elf"
    "pong:pong:pong.elf"
    "rtc:rtc:rtc.elf"
    "ps2_kbd:ps2_kbd:ps2_kbd.elf"
    "virtio_input:virtio_input:virtio_input.elf"
    "compositor:compositor:compositor.elf"
    "ata:ata:ata.elf"
    "ahci:ahci:ahci.elf"
    "usb_host:usb_host:usb_host.elf"
    "usb_storage:usb_storage:usb_storage.elf"
    "usb_hid:usb_hid:usb_hid.elf"
    "virtio_blk:virtio_blk:virtio_blk.elf"
    "nvme:nvme:nvme.elf"
    "logd:logd:logd.elf"
    "ramdisk:ramdisk:ramdisk.elf"
    "vfs_server:vfs_server:vfs_server.elf"
    "usb_serial:usb_serial:usb_serial.elf"
    "uart_serial:uart_serial:uart_serial.elf"
    "link:link:link.elf"
    "blockstore:blockstore:blockstore.elf"
    "blocks:blocks:blocks.elf"
    "tally:tally:tally.elf"
    "fsstep:fsstep:fsstep.elf"
    "storefs:storefs:storefs.elf"
    "gpio:gpio:gpio.elf"
    "battery:battery:battery.elf"
    "loader:loader:loader.elf"
    "audio_gw:audio_gw:audio_gw.elf"
    "tts:tts:tts.elf"
    "video_gw:video_gw:video_gw.elf"
    "virtio_net:virtio_net:virtio_net.elf"
    "rtl8168:rtl8168:rtl8168.elf"
    "rtw89_wifi:rtw89_wifi:rtw89_wifi.elf"
    "rtw88_wifi:rtw88_wifi:rtw88_wifi.elf"
    "bt_hci:bt_hci:bt_hci.elf"
    "bcm_wifi:bcm_wifi:bcm_wifi.elf"
    "wlan:wlan:wlan.elf"
    "netstack:netstack:netstack.elf"
    "netpolicy:netpolicy:netpolicy.elf"
    "timed:timed:timed.elf"
    "parse:parse:parse.elf"
    "nnd:nnd:nnd.elf"
    "tpm:tpm:tpm.elf"
    "netcheck:netcheck:netcheck.elf"
    "nncheck:nncheck:nncheck.elf"
    "webcheck:webcheck:webcheck.elf"
    "effector:effector:effector.elf"
    "effector_gw:effector_gw:effector_gw.elf"
    "download:download:download.elf"
    "get:get:get.elf"
    "windows:windows:windows.elf"
    "wintest:wintest:wintest.elf"
    "wintest:winmgr:winmgr.elf"
    "netbench:netbench:netbench.elf"
    "memtest:memtest:memtest.elf"
    "memtest:memstack:memstack.elf"
    "bench:bench:bench.elf"
    "bench:check:check.elf"
    "bench:kbench:kbench.elf"
    "keystore:keystore:keystore.elf"
    "tls:tls:tls.elf"
    "httpd:httpd:httpd.elf"
    "ssh_service:ssh:ssh.elf"
    "updater:updater:updater.elf"
    "sysmon:sysmon:sysmon.elf"
    "say:say:say.elf"
    "listen:listen:listen.elf"
    "hear:hear:hear.elf"
    "hear:voice:voice.elf"
    "files:files:files.elf"
    "keys:keys:keys.elf"
    "view:view:view.elf"
    "uptime:uptime:uptime.elf"
    "monitor:top:top.elf"
    "monitor:memmap:memmap.elf"
    "monitor:load:load.elf"
    "monitor:hw:hw.elf"
    "monitor:ipc:ipc.elf"
    "monitor:caps:caps.elf"
    "fm:fm:fm.elf"
    "edit:edit:edit.elf"
    "wm:wm:wm.elf"
    "console:console:console.elf"
    "disk:df:df.elf"
    "disk:fsck:fsck.elf"
    "disk:format:format.elf"
    "disk:sha256:sha256.elf"
    "search:find:find.elf"
    "search:grep:grep.elf"
    "dmesg:dmesg:dmesg.elf"
    "svc:svc:svc.elf"
    "beep:beep:beep.elf"
    "record:record:record.elf"
    "pins:pins:pins.elf"
    "pins:pinmap:pinmap.elf"
    "wifi:wifi:wifi.elf"
    "recovery:recovery:recovery.elf"
    "camera:camera:camera.elf"
    "efivar:efivar:efivar.elf"
    "dictate:dictate:dictate.elf"
    "speak:speak:speak.elf"
)

# The kernel, every service and program, and the UEFI bootloader build in parallel, a log each (000-KRN-0020).
source "$BUILD_SCRIPT_DIR/scripts/build_jobs.sh"
JOBS=$(build_jobs)
JOB_LOGS="$LOG_DIR/build"
STEP="installing the pinned toolchain (rust-toolchain.toml)"
ensure_toolchain "$BUILD_SCRIPT_DIR"
echo ">>> [1/2] Building the kernel, the services, the programs and the UEFI bootloader, $JOBS at a time (logs: $JOB_LOGS/)..."
STEP="the cargo builds (the failed ones are named above; logs: $JOB_LOGS/)"
declare -A SEEN=()
{
    printf 'bootloader\t%s\t%s\n' "$BUILD_SCRIPT_DIR/bootloader" "cargo build --release --target x86_64-unknown-uefi"
    for entry in "${USER_CRATES[@]}"; do
        crate_dir=${entry%%:*}
        [ -n "${SEEN[$crate_dir]:-}" ] && continue
        SEEN[$crate_dir]=1
        printf '%s\t%s\t%s\n' "$crate_dir" "$BUILD_SCRIPT_DIR/$crate_dir" "cargo build --release"
    done
} | run_jobs "$JOB_LOGS" "$JOBS"

echo ">>> [2/2] Staging EFI/ELF files..."
STEP="staging artifacts"
mkdir -p "$BUILD_SCRIPT_DIR/usb_root/EFI/BOOT"

for entry in "${USER_CRATES[@]}"; do
    crate_dir=${entry%%:*}
    rest=${entry#*:}
    bin_name=${rest%%:*}
    out_name=${rest#*:}
    # The voice engines are built for targets/x86_64-mind-float.json (SSE2, 250); every other program for x86_64-unknown-none.
    # The newer of the two: a crate that changes its target (tts, 252-APP-0128) leaves the other's last build behind.
    built="$BUILD_SCRIPT_DIR/$crate_dir/target/x86_64-unknown-none/release/$bin_name"
    float="$BUILD_SCRIPT_DIR/$crate_dir/target/x86_64-mind-float/release/$bin_name"
    if [ -f "$float" ] && { [ ! -f "$built" ] || [ "$float" -nt "$built" ]; }; then built=$float; fi
    cp "$built" "$BUILD_SCRIPT_DIR/usb_root/$out_name"
done
cp "$BUILD_SCRIPT_DIR/bootloader/target/x86_64-unknown-uefi/release/bootloader.efi" "$BUILD_SCRIPT_DIR/usb_root/EFI/BOOT/BOOTX64.EFI"
# Licences travel with the image: tts.elf, hear.elf and voice.elf embed third-party dictionaries, the text programs the MIND Mono
# font, tls.elf and keystore.elf BSD and ISC licensed crypto crates (THIRD_PARTY.md). The voice recognizer reads its
# model and grammar from voice/ (issue 078).
mkdir -p "$BUILD_SCRIPT_DIR/usb_root/LICENSES"
cp "$BUILD_SCRIPT_DIR"/LICENSE-MIT "$BUILD_SCRIPT_DIR"/LICENSE-APACHE "$BUILD_SCRIPT_DIR"/THIRD_PARTY.md "$BUILD_SCRIPT_DIR"/LICENSES/*.txt "$BUILD_SCRIPT_DIR/usb_root/LICENSES/"
mkdir -p "$BUILD_SCRIPT_DIR/usb_root/voice"
cp "$BUILD_SCRIPT_DIR"/voice/model.bin "$BUILD_SCRIPT_DIR"/voice/commands.txt "$BUILD_SCRIPT_DIR/usb_root/voice/"
# The runs on real hardware, quick and full, as msh scripts (176-KRN-0087).
mkdir -p "$BUILD_SCRIPT_DIR/usb_root/diag"
cp "$BUILD_SCRIPT_DIR"/diag/*.msh "$BUILD_SCRIPT_DIR/usb_root/diag/"
# Mozilla's public roots as tlsroots.pem, the committed copy checked against its record (000-NET-0046).
STEP="the public root store"
python3 "$BUILD_SCRIPT_DIR/scripts/update_roots.py" --check
cp "$BUILD_SCRIPT_DIR/third_party/roots/cacert.pem" "$BUILD_SCRIPT_DIR/usb_root/tlsroots.pem"
# With $MIND_SECURE_BOOT_KEYS (a directory holding db.key and db.crt), the bootloader is signed for Secure Boot
# first (351-UPD-0012, docs/update/secure-boot.md). Then the boot manifest and its signature: the bootloader loads
# only what it lists (350-UPD-0002, docs/update/README.md).
STEP="signing the boot volume"
if [ -n "${MIND_SECURE_BOOT_KEYS:-}" ]; then
    python3 "$BUILD_SCRIPT_DIR/scripts/secure_boot.py" sign "$BUILD_SCRIPT_DIR/usb_root/EFI/BOOT/BOOTX64.EFI" "$MIND_SECURE_BOOT_KEYS" "$BUILD_SCRIPT_DIR/usb_root/EFI/BOOT/BOOTX64.EFI"
fi
# With $MIND_UPDATE_EVERY (whole seconds), the release carries the updater's period in its slot (351-UPD-0036).
rm -f "$BUILD_SCRIPT_DIR/usb_root/period.txt"
if [ -n "${MIND_UPDATE_EVERY:-}" ]; then
    [[ "$MIND_UPDATE_EVERY" =~ ^[0-9]+$ ]] || { echo "MIND_UPDATE_EVERY must be whole seconds, not '$MIND_UPDATE_EVERY'" >&2; exit 1; }
    printf '# the updater checks this often, in place of update.txt'"'"'s period (351-UPD-0036)\nevery %s\n' "$MIND_UPDATE_EVERY" > "$BUILD_SCRIPT_DIR/usb_root/period.txt"
fi
python3 "$BUILD_SCRIPT_DIR/scripts/sign_manifest.py" "$BUILD_SCRIPT_DIR/usb_root"

trap - ERR
finish
