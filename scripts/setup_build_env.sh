#!/usr/bin/env bash
# Prepares an Ubuntu (22.04, 24.04, or one under WSL2) machine to build MIND Core and run scripts/ci_local.sh: the
# packages CI installs, KVM access where the machine has /dev/kvm, rustup and the pinned toolchain for the builder.
# On 22.04, whose QEMU is 6.2 and Python 3.10: QEMU 8.2.2 (CI's version) from its signed tag on gitlab.com, built into
# /opt, and Python 3.11 in a venv for the host tests. Idempotent: what is there already is left alone, so it may be
# run again at any time. Nothing is rebooted.
#
#   sudo scripts/setup_build_env.sh [--only PART[,PART...]] [USER [TREE]]
#
# PART: packages, kvm, python, rust, qemu (default: all, in this order). USER: who builds (default: the user who ran
# sudo). TREE: a clone whose rust-toolchain.toml is installed for USER (default: the clone this script is in).
set -euo pipefail
export LC_ALL=C   # apt's and the tools' output is parsed: a localized one ("Кандидат:") would read as no candidate

PACKAGES=(
    build-essential pkg-config ca-certificates curl git python3 openssl iproute2   # building, fetching, the netbench tap
    qemu-system-x86 qemu-system-arm qemu-utils ovmf qemu-efi-aarch64 ipxe-qemu     # the QEMU suites, x86 and aarch64
    dosfstools mtools                                                              # disk images (mkfs.fat, mcopy, mtype)
    swtpm swtpm-tools                                                              # the TPM suites
    default-jre-headless                                                           # the TLA+ models (TLC)
    sbsigntool python3-virt-firmware                                               # the Secure Boot groups
    openssh-server acl                                                             # the SSH client's host tests; setfacl
)

PARTS=packages,kvm,python,rust,qemu
if [[ ${1:-} == --only ]]; then PARTS=${2:?--only names parts: packages, kvm, python, rust, qemu}; shift 2; fi
for part in ${PARTS//,/ }; do
    [[ ,packages,kvm,python,rust,qemu, == *",$part,"* ]] || { echo "Unknown part: $part (packages, kvm, python, rust, qemu)" >&2; exit 1; }
done
part() { [[ ,$PARTS, == *",$1,"* ]]; }

[[ $(id -u) == 0 ]] || { echo "Run as root: sudo $0 [--only PART[,PART...]] [USER [TREE]]" >&2; exit 1; }
BUILDER=${1:-${SUDO_USER:-}}
[[ -n $BUILDER && $BUILDER != root ]] || { echo "Name the user who builds: sudo $0 USER" >&2; exit 1; }
id "$BUILDER" >/dev/null
HOME_DIR=$(getent passwd "$BUILDER" | cut -d: -f6)
TREE=${2:-$(cd "$(dirname "$0")/.." && pwd)}
. /etc/os-release
QEMU_VERSION=8.2.2
QEMU_PREFIX=/opt/qemu-$QEMU_VERSION
# The release's signed tag on gitlab.com: download.qemu.org's tarball does not reach every machine (x32-128, 2026-11-02).
QEMU_GIT=https://gitlab.com/qemu-project/qemu.git
QEMU_TAG=v$QEMU_VERSION
QEMU_COMMIT=11aa0b1ff115b86160c4d37e7c37e6a6b13b77ea   # the tag's commit (2024-03-04)
QEMU_SIGNER=6EE195D1886E8FFB810D4324457CE0A0804465C5   # Michael Tokarev, who signs QEMU's stable tags (his primary key)
QEMU_BUILD_PACKAGES=(ninja-build python3-venv python3-tomli python3-distlib flex bison gnupg libglib2.0-dev zlib1g-dev
                     libpixman-1-dev libslirp-dev libfdt-dev libpng-dev libaio-dev)
# Python 3.11 for the host tests where python3 is older; scripts/ci_local.sh puts this venv first for them.
HOST_PYTHON=/opt/mind-python3.11

# Installs the packages not installed yet; one that apt has no candidate for is named, not installed.
install_missing() {
    local missing=() installable=() absent=() package candidate
    for package in "$@"; do
        dpkg-query -W -f='${Status}' "$package" 2>/dev/null | grep -q "install ok installed" || missing+=("$package")
    done
    if ((${#missing[@]} == 0)); then echo ">>> Packages: all there"; return 0; fi
    export DEBIAN_FRONTEND=noninteractive
    apt-get update -qq
    for package in "${missing[@]}"; do
        candidate=$(apt-cache policy "$package" 2>/dev/null | awk '/Candidate:/ {print $2}')
        if [[ -n $candidate && $candidate != "(none)" ]]; then installable+=("$package"); else absent+=("$package"); fi
    done
    if ((${#installable[@]})); then
        echo ">>> Installing: ${installable[*]}"
        apt-get install -y -qq --no-install-recommends "${installable[@]}"
    fi
    if ((${#absent[@]})); then echo "!!! No apt candidate here: ${absent[*]}" >&2; fi
    return 0
}

# 1. Packages: only the missing ones are installed.
part packages && install_missing "${PACKAGES[@]}"

# 2. KVM, where the machine offers it (under WSL2: nested virtualization on). The suites run under TCG either way.
#    The group applies to processes started after it (an agent's service at its next start); the ACL, at once.
if part kvm; then
    if [[ -e /dev/kvm ]]; then
        getent group kvm >/dev/null || groupadd --system kvm
        if ! id -nG "$BUILDER" | grep -qw kvm; then usermod -aG kvm "$BUILDER"; echo ">>> $BUILDER added to the kvm group (for processes started from now)"; fi
        command -v setfacl >/dev/null || install_missing acl
        setfacl -m "u:$BUILDER:rw" /dev/kvm
        runuser -u "$BUILDER" -- test -r /dev/kvm -a -w /dev/kvm && echo ">>> /dev/kvm: $BUILDER can open it now"
    else
        echo ">>> No /dev/kvm: QEMU runs under TCG, as in CI"
    fi
fi

# 3. Python 3.11 or later for the host tests (tomllib). Where python3 is older (22.04's 3.10): Ubuntu's python3.11 in a
#    venv without the system's packages, whose cffi is built for 3.10 only (the manifest test skips cryptography then).
if part python; then
    if python3 -c 'import tomllib' 2>/dev/null; then
        echo ">>> Python: $(python3 --version) has tomllib"
    elif "$HOST_PYTHON/bin/python3" -c 'import tomllib' 2>/dev/null; then
        echo ">>> Python: $("$HOST_PYTHON/bin/python3" --version) there, in $HOST_PYTHON"
    else
        install_missing python3.11 python3.11-venv
        command -v python3.11 >/dev/null || { echo "!!! No python3.11 from apt here (Ubuntu's universe): no venv made" >&2; exit 5; }
        python3.11 -m venv --without-pip "$HOST_PYTHON"
        "$HOST_PYTHON/bin/python3" -c 'import tomllib'
        echo ">>> Python: $("$HOST_PYTHON/bin/python3" --version) in $HOST_PYTHON, for scripts/ci_local.sh's host tests"
    fi
fi

# 4. rustup in the builder's home; the toolchain itself is the one rust-toolchain.toml pins.
if part rust; then
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
fi

# 5. Ubuntu 22.04: QEMU 8.2.2 from its tag, whose signature is checked against the signer's fingerprint and whose commit
#    against the pinned one, installed in /opt and linked into /usr/local/bin ahead of the system's 6.2. Skipped when it
#    is there already.
if part qemu && [[ $VERSION_ID == 22.04 ]]; then
    built=$("$QEMU_PREFIX/bin/qemu-system-x86_64" --version 2>/dev/null || true)
    if [[ $built == *"version $QEMU_VERSION"* && -x $QEMU_PREFIX/bin/qemu-system-aarch64 && -x $QEMU_PREFIX/bin/qemu-img ]]; then
        echo ">>> QEMU $QEMU_VERSION: there, in $QEMU_PREFIX"
    else
        install_missing "${QEMU_BUILD_PACKAGES[@]}"
        work=$(mktemp -d /var/tmp/mind-qemu-build.XXXXXX)
        trap 'rm -rf "$work"' EXIT
        echo ">>> Building QEMU $QEMU_VERSION in $work (several minutes)"
        git clone -q --depth 1 --branch "$QEMU_TAG" "$QEMU_GIT" "$work/qemu"
        export GNUPGHOME=$work/gnupg; mkdir -m 700 "$GNUPGHOME"
        gpg -q --keyserver hkps://keyserver.ubuntu.com --recv-keys "$QEMU_SIGNER"
        verified=$(git -C "$work/qemu" verify-tag --raw "$QEMU_TAG" 2>&1 || true)
        grep -qE "^\[GNUPG:\] VALIDSIG .*$QEMU_SIGNER\$" <<<"$verified" ||
            { echo "!!! QEMU's tag $QEMU_TAG is not signed by $QEMU_SIGNER: not built" >&2; exit 3; }
        [[ $(git -C "$work/qemu" rev-parse "$QEMU_TAG^{commit}") == "$QEMU_COMMIT" ]] ||
            { echo "!!! QEMU's tag $QEMU_TAG is not commit $QEMU_COMMIT: not built" >&2; exit 3; }
        # Out of the source tree. A checkout has no bundled subprojects: meson fetches them from gitlab.com at the
        # revisions the signed tree's .wrap files pin (a missing library stops configure).
        mkdir "$work/build"
        (cd "$work/build" &&
         "$work/qemu/configure" --prefix="$QEMU_PREFIX" --target-list=x86_64-softmmu,aarch64-softmmu \
             --enable-slirp --enable-fdt=system --disable-docs --disable-werror >/dev/null &&
         make -j"$(nproc)" >/dev/null && make install >/dev/null) ||
            { echo "!!! QEMU $QEMU_VERSION did not build (the messages above); nothing was installed in $QEMU_PREFIX" >&2; exit 4; }
        echo ">>> QEMU $QEMU_VERSION installed in $QEMU_PREFIX"
    fi
    for binary in "$QEMU_PREFIX"/bin/*; do ln -sfn "$binary" "/usr/local/bin/$(basename "$binary")"; done
fi

# 6. What scripts/ci_local.sh and the Secure Boot groups look for, after a run of every part.
[[ $PARTS == packages,kvm,python,rust,qemu ]] || { echo ">>> Done: $PARTS"; exit 0; }
qemu_now=$(qemu-system-x86_64 --version 2>/dev/null | head -1)
echo ">>> ${qemu_now:-no qemu-system-x86_64}; free in $HOME_DIR: $(df -h --output=avail "$HOME_DIR" | tail -1 | tr -d ' ')"
absent=()
for tool in python3 mcopy mtype mkfs.fat fsck.fat qemu-img swtpm java openssl sbsign sbvarsign qemu-system-x86_64 qemu-system-aarch64; do
    command -v "$tool" >/dev/null || absent+=("$tool")
done
for file in /usr/share/ovmf/OVMF.fd /usr/share/AAVMF/AAVMF_CODE.fd /usr/share/AAVMF/AAVMF_VARS.fd /usr/sbin/sshd; do [[ -f $file ]] || absent+=("$file"); done
python3 -c "import virt.firmware.varstore, virt.peutils, pefile" 2>/dev/null || absent+=("python3-virt-firmware (import)")
python3 -c 'import tomllib' 2>/dev/null || "$HOST_PYTHON/bin/python3" -c 'import tomllib' 2>/dev/null || absent+=("Python 3.11 (tomllib)")
[[ -x $HOME_DIR/.cargo/bin/cargo ]] || absent+=("cargo for $BUILDER")
if ((${#absent[@]})); then echo "!!! Still missing: ${absent[*]}" >&2; exit 2; fi
echo ">>> Ready: as $BUILDER, '. ~/.cargo/env; scripts/ci_local.sh --list' in a clone"
