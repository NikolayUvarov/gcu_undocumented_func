#!/usr/bin/env bash
# Prepares an Ubuntu (22.04, 24.04, or one under WSL2) machine to build MIND Core and run scripts/ci_local.sh: the
# packages CI installs, KVM access where the machine has /dev/kvm, rustup and the pinned toolchain for the builder.
# Idempotent: what is there already is left alone, so it may be run again at any time. Nothing is rebooted.
#
#   sudo scripts/setup_build_env.sh [USER [TREE]]
#
# USER: who builds (default: the user who ran sudo). TREE: a clone whose rust-toolchain.toml is installed for USER
# (default: the clone this script is in, if it is in one).
set -euo pipefail

PACKAGES=(
    build-essential pkg-config ca-certificates curl git python3 openssl iproute2   # building, fetching, the netbench tap
    qemu-system-x86 qemu-system-arm qemu-utils ovmf qemu-efi-aarch64 ipxe-qemu     # the QEMU suites, x86 and aarch64
    dosfstools mtools                                                              # disk images (mkfs.fat, mcopy, mtype)
    swtpm swtpm-tools                                                              # the TPM suites
    default-jre-headless                                                           # the TLA+ models (TLC)
    sbsigntool python3-virt-firmware                                               # the Secure Boot groups
    openssh-server                                                                 # the SSH client's host tests start their own sshd
)

[[ $(id -u) == 0 ]] || { echo "Run as root: sudo $0 [USER [TREE]]" >&2; exit 1; }
BUILDER=${1:-${SUDO_USER:-}}
[[ -n $BUILDER && $BUILDER != root ]] || { echo "Name the user who builds: sudo $0 USER" >&2; exit 1; }
id "$BUILDER" >/dev/null
HOME_DIR=$(getent passwd "$BUILDER" | cut -d: -f6)
TREE=${2:-$(cd "$(dirname "$0")/.." && pwd)}

# 1. Packages: only the missing ones are installed.
missing=()
for package in "${PACKAGES[@]}"; do
    dpkg-query -W -f='${Status}' "$package" 2>/dev/null | grep -q "install ok installed" || missing+=("$package")
done
if ((${#missing[@]})); then
    echo ">>> Installing: ${missing[*]}"
    export DEBIAN_FRONTEND=noninteractive
    apt-get update -qq
    apt-get install -y -qq --no-install-recommends "${missing[@]}"
else
    echo ">>> Packages: all there"
fi

# 2. KVM, where the machine offers it (under WSL2: nested virtualization on). The suites run under TCG either way.
if [[ -e /dev/kvm ]]; then
    getent group kvm >/dev/null || groupadd --system kvm
    if ! id -nG "$BUILDER" | grep -qw kvm; then usermod -aG kvm "$BUILDER"; echo ">>> $BUILDER added to the kvm group (from the next login)"; fi
else
    echo ">>> No /dev/kvm: QEMU runs under TCG, as in CI"
fi

# 3. rustup in the builder's home; the toolchain itself is the one rust-toolchain.toml pins.
if [[ ! -x $HOME_DIR/.cargo/bin/rustup ]]; then
    echo ">>> Installing rustup for $BUILDER"
    runuser -u "$BUILDER" -- sh -c 'curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain none --no-modify-path'
else
    echo ">>> rustup: there"
fi
if [[ -f $TREE/rust-toolchain.toml ]]; then
    echo ">>> The pinned toolchain for $TREE"
    runuser -u "$BUILDER" -- sh -c "cd '$TREE' && '$HOME_DIR/.cargo/bin/rustup' toolchain install >/dev/null && '$HOME_DIR/.cargo/bin/rustup' show active-toolchain"
fi

# 4. What scripts/ci_local.sh and the Secure Boot groups look for.
absent=()
for tool in python3 mcopy mtype mkfs.fat fsck.fat qemu-img swtpm java openssl sbsign sbvarsign qemu-system-x86_64 qemu-system-aarch64; do
    command -v "$tool" >/dev/null || absent+=("$tool")
done
for file in /usr/share/ovmf/OVMF.fd /usr/share/AAVMF/AAVMF_CODE.fd /usr/share/AAVMF/AAVMF_VARS.fd /usr/sbin/sshd; do [[ -f $file ]] || absent+=("$file"); done
python3 -c "import virt.firmware.varstore, virt.peutils, pefile" 2>/dev/null || absent+=("python3-virt-firmware (import)")
[[ -x $HOME_DIR/.cargo/bin/cargo ]] || absent+=("cargo for $BUILDER")
if ((${#absent[@]})); then echo "!!! Still missing: ${absent[*]}" >&2; exit 2; fi
echo ">>> Ready: as $BUILDER, '. ~/.cargo/env; scripts/ci_local.sh --list' in a clone"
