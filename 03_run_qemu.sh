#!/usr/bin/env bash
# Run MIND CORE in QEMU on Linux after ./02_build.sh (same VM settings as the Windows and WSL launchers).
set -euo pipefail

RUN_SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

fail() {
    printf 'ERROR: %s\n' "$*" >&2
    exit 1
}

QEMU_BIN="${QEMU:-qemu-system-x86_64}"
command -v -- "$QEMU_BIN" >/dev/null 2>&1 || fail "QEMU not found. Install qemu-system-x86 or set QEMU=/path/to/qemu-system-x86_64."

# Firmware: OVMF.fd next to this script, else the split OVMF_CODE/OVMF_VARS of the distribution (pflash, read-only
# code and a private copy of the variables).
FIRMWARE=()
if [[ -f "$RUN_SCRIPT_DIR/OVMF.fd" ]]; then
    FIRMWARE=(-bios "$RUN_SCRIPT_DIR/OVMF.fd")
else
    for code in /usr/share/OVMF/OVMF_CODE_4M.fd /usr/share/OVMF/OVMF_CODE.fd /usr/share/edk2/ovmf/OVMF_CODE.fd /usr/share/qemu/OVMF_CODE.fd; do
        vars="${code/CODE/VARS}"
        if [[ -f "$code" && -f "$vars" ]]; then
            local_vars="$(mktemp --suffix=-OVMF_VARS.fd)"
            cp "$vars" "$local_vars"
            FIRMWARE=(-drive "if=pflash,format=raw,readonly=on,file=$code" -drive "if=pflash,format=raw,file=$local_vars")
            break
        fi
    done
fi
[[ ${#FIRMWARE[@]} -gt 0 ]] || fail "No UEFI firmware: place OVMF.fd next to this script or install the ovmf package."
for artifact in EFI/BOOT/BOOTX64.EFI kernel.elf; do
    [[ -f "$RUN_SCRIPT_DIR/usb_root/$artifact" ]] || fail "Missing usb_root/$artifact. Run ./02_build.sh first."
done

# In -drive a comma separates options; commas in the path are doubled.
USB_ROOT_PATH="${RUN_SCRIPT_DIR//,/,,}/usb_root"

# Sound: audio_gw drives an AC97 controller, so without one `beep`, `say` and `listen` find no device. The host
# backend is the first of PipeWire, PulseAudio, ALSA and SDL that this QEMU has and that starts on this host (QEMU
# stops at once when a backend cannot reach its sound server); MIND_AUDIO=<driver> picks one, none turns the card off.
AUDIO=()
available="$("$QEMU_BIN" -audiodev help 2>/dev/null || true)"
works() { # the backend starts: QEMU is still running when the timeout ends it
    timeout 2 "$QEMU_BIN" -nodefaults -machine none -display none -monitor none -S -audiodev "$1,id=probe" >/dev/null 2>&1
    [[ $? -eq 124 ]]
}
driver="${MIND_AUDIO:-}"
if [[ -z "$driver" ]]; then
    for candidate in pipewire pa alsa sdl; do
        case "$candidate" in # ALSA needs a sound card on this host, SDL a desktop session
            alsa) [[ -e /dev/snd ]] || continue ;;
            sdl) [[ -n "${DISPLAY:-}${WAYLAND_DISPLAY:-}" ]] || continue ;;
        esac
        if grep -qx "$candidate" <<<"$available" && works "$candidate"; then driver="$candidate"; break; fi
    done
    [[ -n "$driver" ]] || printf 'WARNING: no sound: none of pipewire, pa, alsa, sdl starts here (set MIND_AUDIO=<driver>).\n' >&2
fi
if [[ -n "$driver" && "$driver" != none ]]; then
    AUDIO=(-audiodev "$driver,id=snd0" -device AC97,audiodev=snd0)
fi
# Network: a VirtIO card on QEMU's user networking (MIND_NET=none: no card). CPU: RDRAND for the TLS and key services
# (MIND_CPU=<model> to change it).
NET=()
[[ "${MIND_NET:-user}" == none ]] || NET=(-nic "user,model=virtio-net-pci")
# A VirtIO tablet: the system's pointer follows the host's, so it reaches every edge of the screen and the host never
# takes the pointer back halfway (issue 161; MIND_POINTER=ps2: the PS/2 mouse only).
POINTER=()
[[ "${MIND_POINTER:-tablet}" == ps2 ]] || POINTER=(-device virtio-tablet-pci)

printf 'Starting MIND CORE in QEMU: %s (audio: %s)\n' "$QEMU_BIN" "${driver:-none}"
exec "$QEMU_BIN" \
    "${FIRMWARE[@]}" \
    -drive "format=raw,file=fat:rw:$USB_ROOT_PATH" \
    -m 512 -smp 4,sockets=1,cores=4,threads=1 \
    -cpu "${MIND_CPU:-qemu64,+rdrand}" \
    -serial stdio -rtc base=localtime \
    "${AUDIO[@]}" "${NET[@]}" "${POINTER[@]}" \
    "$@"
