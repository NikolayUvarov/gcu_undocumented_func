#!/usr/bin/env bash
# Proprietary files (AGENTS.md, section 3): fetched into proprietary/ (ignored by git), checked by SHA-256, and copied
# onto a written disk's boot volume under data/firmware/, or over the link to a running machine. Never committed, never
# in an image.
# Usage: scripts/proprietary.sh fetch      download, check and extract into proprietary/firmware/
#        scripts/proprietary.sh copy       copy proprietary/firmware/ onto the volume labelled MIND CORE that holds the
#                                          image in usb_root/ (Windows, from WSL), and read it back
#        scripts/proprietary.sh copy DIR   copy it under DIR/data/firmware/ (a mounted boot volume on Linux)
#        scripts/proprietary.sh link PORT [MINDLINK OPTION]...
#                                          put it over MIND Link into the machine's data/firmware/, which link shows as
#                                          firmware/ with `dev` in link.conf (800-LNK-0027); a file held already is skipped
#        scripts/proprietary.sh link effector485 [--reservation SESSION:PIN]
#                                          the same through the effector485 daemon that holds the port ($EFFECTOR485, else
#                                          effector485 on the PATH)
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
    python3 "$ROOT/scripts/firmware/rtl8852be.py" fetch
    python3 "$ROOT/scripts/firmware/rtl8821ce.py" fetch
    python3 "$ROOT/scripts/firmware/rtl8821c_bt.py" fetch
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

# Copied by disk_copy.sh, which waits for the volume of the image just built and reads each file back.
copy() {
    [[ -d $STORE/firmware ]] || fail "nothing fetched: run $0 fetch first"
    exec "$ROOT/scripts/disk_copy.sh" "$STORE/firmware" data/firmware proprietary "$@"
}

# Put over the link (800-LNK-0027): each file under firmware/ by its path in the store, its directories made first.
# mindlink skips a file whose SHA-256 the other end has; the daemon's put does the same.
link() {
    [[ -d $STORE/firmware ]] || fail "nothing fetched: run $0 fetch first"
    local port="${1:?usage: $0 link PORT [MINDLINK OPTION]... | $0 link effector485 [--reservation SESSION:PIN]}"; shift
    local files=() dirs=() f rel
    while IFS= read -r -d '' f; do files+=("${f#"$STORE/firmware/"}"); done < <(find "$STORE/firmware" -type f -print0 | sort -z)
    [[ ${#files[@]} -gt 0 ]] || fail "no file in $STORE/firmware"
    for rel in "${files[@]}"; do [[ $rel == */* ]] && dirs+=("${rel%/*}"); done
    mapfile -t dirs < <(printf '%s\n' "${dirs[@]}" | sort -u)
    if [[ $port == effector485 ]]; then
        local exe="${EFFECTOR485:-effector485}" key="proprietary-$(date +%s)-$$" reason="proprietary.sh link: drivers' firmware into data/firmware/ (800-LNK-0027)" n=0
        local winpath=(); command -v wslpath >/dev/null && winpath=(wslpath -w)
        for rel in "${dirs[@]}"; do "$exe" submit mkdir "name=firmware/$rel" "$@" --key "$key-d$((n++))" --reason "$reason" --wait >/dev/null; done
        for rel in "${files[@]}"; do
            local here="$STORE/firmware/$rel"; [[ ${#winpath[@]} -gt 0 ]] && here="$("${winpath[@]}" "$here")"
            "$exe" submit put "file=$here" "name=firmware/$rel" "$@" --key "$key-f$((n++))" --reason "$reason" --wait
        done
    else
        local mindlink="${MINDLINK:-mindlink}"
        for rel in "${dirs[@]}"; do "$mindlink" "$@" "$port" mkdir "firmware/$rel" >/dev/null; done
        for rel in "${files[@]}"; do "$mindlink" "$@" "$port" put "$STORE/firmware/$rel" "firmware/$rel"; done
    fi
    echo "proprietary: ${#files[@]} files put into firmware/ over the link; a driver reads its file at its next start"
}

case "${1:-}" in
    fetch) fetch ;;
    copy) shift; copy "$@" ;;
    link) shift; link "$@" ;;
    *) sed -n '2,14p' "$0" | sed 's/^# \{0,1\}//'; exit 2 ;;
esac
