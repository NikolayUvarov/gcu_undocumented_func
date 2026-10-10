#!/usr/bin/env bash
# Proprietary files (AGENTS.md, section 3): fetched into proprietary/ (ignored by git), checked by SHA-256, and copied
# onto a written disk's boot volume under data/firmware/. Never committed, never in an image.
# Usage: scripts/proprietary.sh fetch      download, check and extract into proprietary/firmware/
#        scripts/proprietary.sh copy       copy proprietary/firmware/ onto the volume labelled MIND CORE (Windows, from WSL)
#        scripts/proprietary.sh copy DIR   copy it under DIR/data/firmware/ (a mounted boot volume on Linux)
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
STORE="$ROOT/proprietary"

# Broadcom's 802.11 microcode for the MacBook Pro's BCM4331 (bcm_wifi, 550-DRV-0006): core revision 29, HT-PHY.
# The archive's first home (lwfinger.com) is gone; distributions moved to this copy. Its SHA-256 is nixpkgs's and its
# MD5 SlackBuilds's: both published apart from the copy.
B43_URL="https://github.com/minios-linux/b43-firmware/releases/download/b43-firmware/broadcom-wl-6.30.163.46.tar.bz2"
B43_SHA256="a07c3b6b277833c7dbe61daa511f908cd66c5e2763eb7a0859abc36cd9335c2d"
B43_MD5="6fe97e9368d25342a1ab943d3cf3496d"
B43_OBJECT="broadcom-wl-6.30.163.46.wl_apsta.o"
# The files bcm_wifi loads, and their SHA-256 as b43-fwcutter 019 extracts them.
B43_FILES=(
    "ucode29_mimo.fw df16911e9ec2ae2173bcdea06e49977b544ba666222c5e5fab5be0cca47bce03"
    "ht0initvals29.fw 1ee846e61c27505bc55d68bd1ff8155affc2771eb8244d935d329ed8c08ad764"
    "ht0bsinitvals29.fw c45a240b9bb97df9f99550abe4a2904908931c5266cf7874869e8cdf6db8a7c5"
)

fail() { echo "proprietary: $*" >&2; exit 1; }

fetch() {
    command -v b43-fwcutter >/dev/null || fail "b43-fwcutter is not installed (sudo apt-get install b43-fwcutter)"
    mkdir -p "$STORE/downloads" "$STORE/firmware/b43"
    local archive="$STORE/downloads/${B43_URL##*/}"
    if [[ ! -f $archive ]]; then curl -fsSL --retry 3 -o "$archive.part" "$B43_URL"; mv "$archive.part" "$archive"; fi
    [[ $(sha256sum "$archive" | cut -d' ' -f1) == "$B43_SHA256" ]] || { rm -f "$archive"; fail "SHA-256 of ${archive##*/} does not match: removed"; }
    [[ $(md5sum "$archive" | cut -d' ' -f1) == "$B43_MD5" ]] || { rm -f "$archive"; fail "MD5 of ${archive##*/} does not match: removed"; }
    local work; work="$(mktemp -d)"; trap 'rm -rf "$work"' RETURN
    tar -xjf "$archive" -C "$work" "$B43_OBJECT"
    mkdir -p "$work/out" && b43-fwcutter -w "$work/out" "$work/$B43_OBJECT" >/dev/null
    for entry in "${B43_FILES[@]}"; do
        local name="${entry% *}" sum="${entry#* }"
        [[ $(sha256sum "$work/out/b43/$name" | cut -d' ' -f1) == "$sum" ]] || fail "$name extracted with another SHA-256"
        cp "$work/out/b43/$name" "$STORE/firmware/b43/$name"
    done
    echo "proprietary: ${#B43_FILES[@]} files in $STORE/firmware/b43, checked"
}

copy() {
    [[ -d $STORE/firmware ]] || fail "nothing fetched: run $0 fetch first"
    if [[ $# -ge 1 ]]; then
        mkdir -p "$1/data/firmware" && cp -r "$STORE/firmware/." "$1/data/firmware/"
        echo "proprietary: copied under $1/data/firmware"
        return
    fi
    command -v powershell.exe >/dev/null || fail "no PowerShell: give the mounted boot volume as an argument"
    local source; source="$(wslpath -w "$STORE/firmware")"
    # Windows mounts the written disk's volumes a few seconds after the writer ends: wait up to 30 s for MIND CORE.
    powershell.exe -NoProfile -Command "for (\$i = 0; \$i -lt 15; \$i++) { \$v = Get-Volume | Where-Object FileSystemLabel -eq 'MIND CORE' | Where-Object DriveLetter | Select-Object -First 1; if (\$v) { break }; Start-Sleep 2 }; if (-not \$v) { Write-Error 'No volume labelled MIND CORE with a drive letter'; exit 1 }; \$to = \$v.DriveLetter + ':\\data\\firmware'; New-Item -ItemType Directory -Force \$to | Out-Null; Copy-Item -Recurse -Force '$source\\*' \$to; Write-Output ('proprietary: copied to ' + \$to)" | tr -d '\r'
}

case "${1:-}" in
    fetch) fetch ;;
    copy) shift; copy "$@" ;;
    *) sed -n '2,7p' "$0" | sed 's/^# \{0,1\}//'; exit 2 ;;
esac
