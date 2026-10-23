#!/usr/bin/env bash
# Pull a branch (or take one commit), build MIND Core, pack the USB image, then write it to a disk from an elevated
# Windows PowerShell: the maintainer's one step from WSL to a written USB disk (docs/write-disk.md, section 5).
# The clone it builds is the repository it lives in, also when it is started through a symbolic link from outside.
# Usage: scripts/build_and_write.sh [--commit <id> | --image <file>] [--slots] [--wifitest] [--wifi-ofdm[=<cores off>]] [--wifijoin] [--link[=<baud>]] [--link-run] [--link-dev] [--keep-wifi] [--mode=<modes>] <disk number>   |   --list
#   --commit <id>  build that commit (a short or full hash, a tag) instead of the branch's latest; the clone returns
#                  to the branch when the script ends, so this script stays in it.
#   --image <file> write that image instead of building one (a trial built elsewhere); the image's MANIFEST must lie
#                  beside it, so the steps after the write find its volume. Proprietary files and wallpapers are
#                  not copied then.
#   --slots        the image with boot slots (docs/update/slots.md) and an update.txt at its root that takes releases
#                  from data/link/rel/ (351-KRN-0141): after this one write, scripts/link_update.py updates the machine
#                  over the link, with no rewrite. It writes link.conf as --link does unless a --link option is given.
#   --wifitest     after the write, put data/wifitest.txt on the boot volume: bcm_wifi then runs its Wi-Fi test stages.
#   --wifi-ofdm[=<cores off>]  after the write, put data/wifi-ofdm.txt on the boot volume: bcm_wifi then offers the
#                  station 802.11g and counts OFDM received, good and with a bad FCS, by rate and length (550-DRV-0029).
#                  With a mask 0 to 7, the file says `cores-off=<mask>`: those cores' front ends are switched off in
#                  place of core 2's alone (core 0 is bit 0; 6 leaves core 0 alone, 0 every core on).
#   --wifijoin     after the write, put data/wifijoin.txt (the network's name, then its passphrase, from
#                  ~/.config/mind/wifijoin, mode 600) on the boot volume and, when the tree has it,
#                  tests/hardware/wifijoin.msh as autorun.msh at its root, which the shell runs once at start
#                  (550-ASR-0007, 000-APP-0079). The passphrase is never printed.
#   --link[=<baud>] after the write, put link.conf (`baud=<baud>`, default 460800) at the boot volume's root: init then
#                  starts usb_serial and link (800-LNK-0003, once 800-KRN-0128 is in the build), so mindlink on another
#                  machine reads the log and moves files over a USB-serial converter (docs/link/mindlink.md).
#   --link-run     as --link, with the word `run` in link.conf: link then starts programs from data/link/ for the
#                  other machine (`mindlink run`, effector485; 800-LNK-0009), once init gives link its endpoint.
#   --link-dev     as --link-run, with the word `dev` too: the developer mode (800-LNK-0012), the maintainer's decision
#                  of 2026-10-21: link may also give the programs it starts the network and the Wi-Fi station, run the
#                  built-in programs and restart the machine, once init grants it these (KRN). Whoever holds the cable
#                  then debugs the machine as its user would. For the maintainer's own machines only.
#   --mode=<modes> after the write, put data/mode.conf on the boot volume with the modes, one line: `maintenance`,
#                  `development`, or both comma-separated (`--mode=development,maintenance`). init reads it at start
#                  and runs the Effector agent's services in them (750-KRN-0143, docs/effector/README.md); `mode`
#                  in the shell changes it later.
#   --keep-wifi    keep the Wi-Fi networks the machine remembers across the write: before it, system/keystore/
#                  (wifi.kept, device.key, wifi.country) is saved from the disk's volume MIND CORE into
#                  ~/.config/mind/keystore (mode 600; a fresh disk uses what was saved last), and after it put back,
#                  with data/wifijoin.txt holding the network's name alone (the first line of ~/.config/mind/wifijoin,
#                  or KEEP_WIFI_NAME) and tests/hardware/wifijoin.msh as autorun.msh: the shell joins it at start, no
#                  passphrase on the disk. The saved files hold the network's key: never printed or committed.
#                  The network policy as changed on the machine (system/netpolicy/netpolicy.txt, `netpolicy add`) is
#                  kept too, in ~/.config/mind/netpolicy: put back as the new image's shipped policy with the same
#                  lines added and removed, so a rule like `netcheck <gateway> icmp` need not be typed again.
set -euo pipefail

# The clone changes under the script (checkout, pull), and bash reads a script as it runs: so it runs from a copy.
if [[ -z ${BUILD_AND_WRITE_COPY:-} ]]; then
    self="$(readlink -f -- "${BASH_SOURCE[0]}")"
    copy="$(mktemp "${TMPDIR:-/tmp}/build_and_write.XXXXXX")"
    cp -- "$self" "$copy"
    BUILD_AND_WRITE_SELF="$self" BUILD_AND_WRITE_COPY="$copy" exec bash "$copy" "$@"
fi
SELF="${BUILD_AND_WRITE_SELF:-}" COPY="$BUILD_AND_WRITE_COPY"
export -n BUILD_AND_WRITE_SELF BUILD_AND_WRITE_COPY
# Only the copy itself is ever removed.
[[ ${BASH_SOURCE[0]} == "$COPY" && $COPY != "$SELF" ]] || { echo "build_and_write: not running from its copy ($COPY)" >&2; exit 1; }
trap 'rm -f -- "$COPY"' EXIT

REPO="${REPO:-$(git -C "$(dirname -- "$SELF")" rev-parse --show-toplevel)}"
BRANCH="${BRANCH:-fast-test}"
WRITER_WIN="$(wslpath -w "$REPO/05_write_usb_windows.ps1")"

# Runs PowerShell code passed as base64 UTF-16LE, so no quoting is lost between WSL and Windows.
winps() { powershell.exe -NoProfile -ExecutionPolicy Bypass -EncodedCommand "$(printf '%s' "$1" | iconv -t UTF-16LE | base64 -w0)"; }

COMMIT=""; FLAGS=(); SLOTS=0; WIFIJOIN=0; LINK_BAUD=; LINK_RUN=0; LINK_DEV=0; KEEP_WIFI=0; IMAGE_FILE=
while [[ $# -gt 0 ]]; do
    case "$1" in
        --commit) COMMIT="${2:?usage: $0 --commit <id> <disk number>}"; shift 2 ;;
        --image) IMAGE_FILE="$(realpath "${2:?usage: $0 --image <file> <disk number>}")"; shift 2 ;;
        --slots) SLOTS=1; shift ;;
        --wifitest) FLAGS+=(wifitest.txt); shift ;;
        --wifi-ofdm) FLAGS+=(wifi-ofdm.txt); shift ;;
        --wifi-ofdm=*) m="${1#--wifi-ofdm=}"; [[ "$m" =~ ^[0-7]$ ]] || { echo "--wifi-ofdm=<cores off>: a mask 0 to 7" >&2; exit 1; }
                       FLAGS+=("wifi-ofdm.txt:cores-off=$m"); shift ;;
        --wifijoin) WIFIJOIN=1; shift ;;
        --link) LINK_BAUD=460800; shift ;;
        --link=*) LINK_BAUD="${1#--link=}"; [[ "$LINK_BAUD" =~ ^[0-9]+$ ]] || { echo "--link=<baud>: digits" >&2; exit 1; }; shift ;;
        --link-run) LINK_RUN=1; shift ;;
        --link-dev) LINK_RUN=1; LINK_DEV=1; shift ;;
        --keep-wifi) KEEP_WIFI=1; shift ;;
        --mode=*) m="${1#--mode=}"; words=()
                  for w in ${m//,/ }; do
                      [[ $w == development || $w == maintenance ]] || { echo "--mode=<modes>: development, maintenance or both, comma-separated: $w" >&2; exit 1; }
                      [[ " ${words[*]-} " == *" $w "* ]] || words+=("$w")
                  done
                  [[ ${#words[@]} -gt 0 ]] || { echo "--mode=<modes>: development, maintenance or both" >&2; exit 1; }
                  FLAGS+=("mode.conf:${words[*]}"); shift ;;
        *) break ;;
    esac
done

# Request files (data/NAME, holding TEXT) put on the volume MIND CORE that holds this build's MANIFEST (at its root, or in
# MIND/A with --slots), flushed and read back.
read -r -d '' PS_FLAG <<'PS' || true
$ErrorActionPreference = 'Stop'; $ProgressPreference = 'SilentlyContinue'; trap { Write-Output "flag: $_"; exit 1 }
for ($i = 0; $i -lt 30; $i++) {
    $v = Get-Volume | Where-Object { $_.FileSystemLabel -eq 'MIND CORE' -and $_.DriveLetter } | Select-Object -First 1
    if ($v -and @("$($v.DriveLetter):\MANIFEST", "$($v.DriveLetter):\MIND\A\MANIFEST" | Where-Object { (Test-Path $_) -and (Get-FileHash $_).Hash -eq '__MANIFEST__' }).Count) { break }
    $v = $null; Start-Sleep 2
}
if (-not $v) { Write-Output 'flag: no volume MIND CORE holding this build (its MANIFEST) within 60 s'; exit 1 }
$f = "$($v.DriveLetter):\data\__NAME__"
New-Item -ItemType Directory -Force (Split-Path $f) | Out-Null
Set-Content -Path $f -Value '__TEXT__' -Encoding ascii
Write-VolumeCache -DriveLetter $v.DriveLetter
if (Test-Path $f) { Write-Output "flag: $f written" } else { Write-Output "flag: $f NOT written"; exit 1 }
PS
# A flag is NAME, or NAME:TEXT for a file that says more than that it was asked for.
put_flag() {
    local name="${1%%:*}" text='requested by build-and-write.sh'
    [[ $1 == *:* ]] && text="${1#*:}"
    local script="${PS_FLAG//__MANIFEST__/$(sha256sum "$MANIFEST_FILE" | cut -d' ' -f1)}"
    script="${script//__TEXT__/$text}"
    winps "${script//__NAME__/$name}" | tr -d '\r'
}

# The Wi-Fi network to join and its passphrase (550-ASR-0007), from the maintainer's own file: written through WSL's
# mount of the volume holding this build, so the passphrase is on no command line; read back; never printed.
WIFIJOIN_CONF="${WIFIJOIN_CONF:-$HOME/.config/mind/wifijoin}"
read -r -d '' PS_LETTER <<'PS' || true
$ErrorActionPreference = 'Stop'; $ProgressPreference = 'SilentlyContinue'
for ($i = 0; $i -lt 30; $i++) {
    $v = Get-Volume | Where-Object { $_.FileSystemLabel -eq 'MIND CORE' -and $_.DriveLetter } | Select-Object -First 1
    if ($v -and @("$($v.DriveLetter):\MANIFEST", "$($v.DriveLetter):\MIND\A\MANIFEST" | Where-Object { (Test-Path $_) -and (Get-FileHash $_).Hash -eq '__MANIFEST__' }).Count) { Write-Output $v.DriveLetter; exit 0 }
    Start-Sleep 2
}
exit 1
PS
put_wifijoin() {
    [[ -f $WIFIJOIN_CONF ]] || { echo "wifijoin: no $WIFIJOIN_CONF (its first line the network's name, its second the passphrase)" >&2; return 1; }
    local letter dir status=0
    letter=$(winps "${PS_LETTER//__MANIFEST__/$(sha256sum "$MANIFEST_FILE" | cut -d' ' -f1)}" | tr -d '\r\n')
    [[ $letter =~ ^[A-Z]$ ]] || { echo "wifijoin: no volume MIND CORE holding this build (its MANIFEST) within 60 s" >&2; return 1; }
    dir=$(mktemp -d)
    sudo mount -t drvfs "$letter:" "$dir" -o "uid=$(id -u),gid=$(id -g)" || { rmdir "$dir"; return 1; }
    mkdir -p "$dir/data" && cp "$WIFIJOIN_CONF" "$dir/data/wifijoin.txt" && sync
    if cmp -s "$WIFIJOIN_CONF" "$dir/data/wifijoin.txt"; then
        echo "wifijoin: $letter:\\data\\wifijoin.txt written for the network \"$(head -1 "$WIFIJOIN_CONF")\", read back"
    else echo "wifijoin: $letter:\\data\\wifijoin.txt NOT written" >&2; status=1; fi
    if [[ -f tests/hardware/wifijoin.msh ]]; then
        if cp tests/hardware/wifijoin.msh "$dir/autorun.msh" && sync && cmp -s tests/hardware/wifijoin.msh "$dir/autorun.msh"; then
            echo "wifijoin: $letter:\\autorun.msh written from tests/hardware/wifijoin.msh, read back"
        else echo "wifijoin: $letter:\\autorun.msh NOT written" >&2; status=1; fi
    else echo "wifijoin: no tests/hardware/wifijoin.msh in this tree yet: join by hand with wifi"; fi
    sudo umount "$dir"; rmdir "$dir"
    winps "\$ProgressPreference = 'SilentlyContinue'; Write-VolumeCache -DriveLetter $letter" > /dev/null || true
    return $status
}

# link.conf at the root of the volume holding this build: the link's rate, the words init passes to link.
read -r -d '' PS_LINK <<'PS' || true
$ErrorActionPreference = 'Stop'; $ProgressPreference = 'SilentlyContinue'; trap { Write-Output "link: $_"; exit 1 }
for ($i = 0; $i -lt 30; $i++) {
    $v = Get-Volume | Where-Object { $_.FileSystemLabel -eq 'MIND CORE' -and $_.DriveLetter } | Select-Object -First 1
    if ($v -and @("$($v.DriveLetter):\MANIFEST", "$($v.DriveLetter):\MIND\A\MANIFEST" | Where-Object { (Test-Path $_) -and (Get-FileHash $_).Hash -eq '__MANIFEST__' }).Count) { break }
    $v = $null; Start-Sleep 2
}
if (-not $v) { Write-Output 'link: no volume MIND CORE holding this build (its MANIFEST) within 60 s'; exit 1 }
$f = "$($v.DriveLetter):\link.conf"
Set-Content -Path $f -Value '__CONF__' -Encoding ascii
Write-VolumeCache -DriveLetter $v.DriveLetter
if ((Get-Content $f) -eq '__CONF__') { Write-Output "link: $f written (__CONF__), read back" } else { Write-Output "link: $f NOT written"; exit 1 }
PS
put_link() {
    local script="${PS_LINK//__MANIFEST__/$(sha256sum "$MANIFEST_FILE" | cut -d' ' -f1)}"
    local conf="baud=$1"; [[ $LINK_RUN == 1 ]] && conf+=" run"; [[ $LINK_DEV == 1 ]] && conf+=" dev"
    winps "${script//__CONF__/$conf}" | tr -d '\r'
}

# The remembered Wi-Fi networks (keystore's files, 550-NET-0024), carried across a write through WSL's mount of the
# volume, so their bytes are on no command line; the mount is undone at once (the writer needs the volume unlocked).
KEEP_DIR="${KEEP_DIR:-$HOME/.config/mind/keystore}"
KEPT_FILES=(wifi.kept device.key wifi.country)
# The network policy as the owner changed it (108), with the shipped policy it was changed from.
POLICY_KEEP="${POLICY_KEEP:-$HOME/.config/mind/netpolicy}"
save_keystore() {
    local letter dir n=0
    letter=$(winps "\$ProgressPreference = 'SilentlyContinue'; \$p = Get-Partition -DiskNumber $DISK -ErrorAction SilentlyContinue | Get-Volume | Where-Object { \$_.FileSystemLabel -eq 'MIND CORE' -and \$_.DriveLetter } | Select-Object -First 1; if (\$p) { \$p.DriveLetter }" | tr -d '\r\n')
    if [[ ! $letter =~ ^[A-Z]$ ]]; then echo "keep-wifi: disk $DISK has no volume MIND CORE now; $(ls "$KEEP_DIR" 2>/dev/null | wc -l) file(s) saved before will be put back"; return 0; fi
    dir=$(mktemp -d)
    sudo mount -t drvfs "$letter:" "$dir" -o "uid=$(id -u),gid=$(id -g)" || { rmdir "$dir"; return 1; }
    if [[ -f $dir/system/keystore/wifi.kept ]]; then
        mkdir -p "$KEEP_DIR" && chmod 700 "$KEEP_DIR"
        for f in "${KEPT_FILES[@]}"; do
            if [[ -f $dir/system/keystore/$f ]]; then (umask 077; cp "$dir/system/keystore/$f" "$KEEP_DIR/$f") && chmod 600 "$KEEP_DIR/$f" && n=$((n + 1)); fi
        done
        echo "keep-wifi: $n file(s) of $letter:\\system\\keystore saved in $KEEP_DIR"
    else
        echo "keep-wifi: $letter: remembers no Wi-Fi network; $(ls "$KEEP_DIR" 2>/dev/null | wc -l) file(s) saved before will be put back"
    fi
    if [[ -f $dir/system/netpolicy/netpolicy.txt ]]; then
        mkdir -p "$POLICY_KEEP" && chmod 700 "$POLICY_KEEP"
        tr -d '\r' < "$dir/system/netpolicy/netpolicy.txt" > "$POLICY_KEEP/changed.txt"
        if [[ -f $dir/netpolicy.txt ]]; then tr -d '\r' < "$dir/netpolicy.txt" > "$POLICY_KEEP/shipped.txt"; else : > "$POLICY_KEEP/shipped.txt"; fi
        echo "keep-wifi: the network policy as changed on $letter: saved in $POLICY_KEEP ($(policy_changes added) added, $(policy_changes removed) removed)"
    elif [[ -f $POLICY_KEEP/changed.txt ]]; then
        echo "keep-wifi: $letter: the network policy is not changed there; the changes saved before will be put back"
    fi
    sudo umount "$dir"; rmdir "$dir"
}
# How many lines the saved policy adds to (`added`) or removes from (`removed`) the shipped one it was changed from.
policy_changes() {
    local from="$POLICY_KEEP/shipped.txt" to="$POLICY_KEEP/changed.txt"
    [[ $1 == added ]] || { from="$POLICY_KEEP/changed.txt"; to="$POLICY_KEEP/shipped.txt"; }
    awk 'FILENAME == ARGV[1] { s[$0] = 1; next } !($0 in s)' "$from" "$to" | wc -l
}
# The policy to put back: the new image's shipped lines but those the owner removed, then the lines the owner added.
merge_policy() {
    awk 'FILENAME == ARGV[1] { old[$0] = 1; next }
         FILENAME == ARGV[2] { mine[$0] = 1; order[++n] = $0; next }
         { sub(/\r$/, ""); now[$0] = 1; if (!($0 in old) || ($0 in mine)) print }
         END { for (i = 1; i <= n; i++) if (!(order[i] in old) && !(order[i] in now)) print order[i] }' \
        "$POLICY_KEEP/shipped.txt" "$POLICY_KEEP/changed.txt" "$1"
}
put_keystore() {
    [[ -f $KEEP_DIR/wifi.kept ]] || { echo "keep-wifi: nothing saved in $KEEP_DIR: join by hand once, then write with --keep-wifi" >&2; return 1; }
    local letter dir status=0 f
    letter=$(winps "${PS_LETTER//__MANIFEST__/$(sha256sum "$MANIFEST_FILE" | cut -d' ' -f1)}" | tr -d '\r\n')
    [[ $letter =~ ^[A-Z]$ ]] || { echo "keep-wifi: no volume MIND CORE holding this build (its MANIFEST) within 60 s" >&2; return 1; }
    dir=$(mktemp -d)
    sudo mount -t drvfs "$letter:" "$dir" -o "uid=$(id -u),gid=$(id -g)" || { rmdir "$dir"; return 1; }
    mkdir -p "$dir/system/keystore"
    for f in "${KEPT_FILES[@]}"; do
        [[ -f $KEEP_DIR/$f ]] || continue
        cp "$KEEP_DIR/$f" "$dir/system/keystore/$f" && sync && cmp -s "$KEEP_DIR/$f" "$dir/system/keystore/$f" || { echo "keep-wifi: $f NOT put back" >&2; status=1; }
    done
    [[ $status == 0 ]] && echo "keep-wifi: the remembered Wi-Fi networks put back on $letter:\\system\\keystore, read back"
    if [[ -f $POLICY_KEEP/changed.txt ]]; then
        local policy; policy=$(mktemp)
        merge_policy "$([[ -f $dir/netpolicy.txt ]] && echo "$dir/netpolicy.txt" || echo /dev/null)" > "$policy"
        mkdir -p "$dir/system/netpolicy" && cp "$policy" "$dir/system/netpolicy/netpolicy.txt" && sync \
            && cmp -s "$policy" "$dir/system/netpolicy/netpolicy.txt" \
            && echo "keep-wifi: the network policy put back on $letter:\\system\\netpolicy, $(grep -c . "$policy") line(s), read back" \
            || { echo "keep-wifi: the network policy NOT put back" >&2; status=1; }
        rm -f "$policy"
    fi
    # The join at start: the network's name alone (no passphrase: keystore holds the key) and the tree's wifijoin.msh
    # as autorun.msh (550-ASR-0007); --wifijoin's files, with the passphrase, take its place when it is given too.
    local name="${KEEP_WIFI_NAME:-$(head -1 "$WIFIJOIN_CONF" 2>/dev/null | tr -d '\r')}"
    if [[ $WIFIJOIN == 1 ]]; then :
    elif [[ -z $name ]]; then echo "keep-wifi: no network's name (KEEP_WIFI_NAME, or the first line of $WIFIJOIN_CONF): no join at start" >&2
    elif ! grep -q 'REMEMBERED BY KEYSTORE' tests/hardware/wifijoin.msh 2>/dev/null; then echo "keep-wifi: this tree's wifijoin.msh cannot join a remembered network yet: join by hand" >&2
    else
        mkdir -p "$dir/data" && printf '%s\n' "$name" > "$dir/data/wifijoin.txt" && cp tests/hardware/wifijoin.msh "$dir/autorun.msh" && sync \
            && cmp -s tests/hardware/wifijoin.msh "$dir/autorun.msh" \
            && echo "keep-wifi: autorun.msh joins \"$name\" at start (data/wifijoin.txt holds the name alone)" \
            || { echo "keep-wifi: the join at start NOT written" >&2; status=1; }
    fi
    sudo umount "$dir"; rmdir "$dir"
    winps "\$ProgressPreference = 'SilentlyContinue'; Write-VolumeCache -DriveLetter $letter" > /dev/null || true
    return $status
}

if [[ "${1:-}" == "--list" ]]; then
    winps "\$ProgressPreference = 'SilentlyContinue'; & '$WRITER_WIN' -List" | tr -d '\r'
    exit 0
fi
[[ ($LINK_RUN == 1 || $SLOTS == 1) && -z $LINK_BAUD ]] && LINK_BAUD=460800
[[ $SLOTS == 1 && -n $IMAGE_FILE ]] && { echo "--slots builds the image; it does not go with --image" >&2; exit 1; }
DISK="${1:?usage: build_and_write.sh [--commit <id>] [--slots] [--wifitest] [--wifi-ofdm[=<cores off>]] [--wifijoin] [--link[=<baud>]] [--link-run] [--link-dev] [--keep-wifi] [--mode=<modes>] <disk number> | --list}"
[[ "$DISK" =~ ^[0-9]+$ ]] || { echo "Disk number must be digits: $DISK" >&2; exit 1; }

cd "$REPO"
# The volume after the write is found by its MANIFEST: this build's, or the given image's beside it.
MANIFEST_FILE="$REPO/usb_root/MANIFEST"
HAVE_PROPRIETARY=0
if [[ -n $IMAGE_FILE ]]; then
    [[ -f $IMAGE_FILE ]] || { echo "No image $IMAGE_FILE" >&2; exit 1; }
    MANIFEST_FILE="$(dirname "$IMAGE_FILE")/MANIFEST"
    [[ -f $MANIFEST_FILE ]] || { echo "No MANIFEST beside $IMAGE_FILE: the steps after the write could not find its volume" >&2; exit 1; }
    echo ">>> Writing the given image $IMAGE_FILE (no build); its MANIFEST $(sha256sum "$MANIFEST_FILE" | cut -c1-16)"
else
git fetch origin
if [[ -n "$COMMIT" ]]; then
    git checkout --detach "$COMMIT"
    trap 'git -C "$REPO" checkout -q "$BRANCH" || echo ">>> The clone stays at $COMMIT: git checkout $BRANCH there." >&2; rm -f -- "$COPY"' EXIT
    echo ">>> Building commit $(git log -1 --format='%h %ad %s' --date=short)"
else
    git checkout "$BRANCH"
    git pull --ff-only
fi
# Build first, then pack: make_usb_image.py lists the programs when it starts, so packing inside its own build misses new ones.
bash ./02_build.sh
# With --slots: the build in slot A and an update.txt that takes releases from data/link/rel/ (scripts/link_update.py).
SLOT_ARGS=()
if [[ $SLOTS == 1 ]]; then
    UPDATE_TXT="$(mktemp)"
    printf 'source data/link/rel\nchannel stable\nautomatic apply\nevery 20\n' > "$UPDATE_TXT"
    SLOT_ARGS=(--slots --update "$UPDATE_TXT")
    echo ">>> An image with slots; update.txt takes releases from data/link/rel/ (scripts/link_update.py after this write)"
fi
python3 scripts/make_usb_image.py --no-build --force ${SLOT_ARGS[@]+"${SLOT_ARGS[@]}"}

# Proprietary files (AGENTS.md, section 3): fetched into proprietary/ and checked now, copied onto the disk after the write.
if [[ -x scripts/proprietary.sh ]]; then scripts/proprietary.sh fetch && HAVE_PROPRIETARY=1; fi
fi

IMAGE_WIN="$(wslpath -w "${IMAGE_FILE:-$REPO/dist/mind-core-usb.img}")"
case "$IMAGE_WIN" in
    \\\\*) echo "The image must be on a Windows drive (/mnt/c/...), not in the WSL file system: $IMAGE_WIN" >&2; exit 1 ;;
esac

if [[ $KEEP_WIFI == 1 ]]; then save_keystore || echo ">>> The remembered Wi-Fi networks NOT saved from disk $DISK." >&2; fi
echo ">>> Opening an elevated PowerShell to write $IMAGE_WIN to disk $DISK (UAC, then type ERASE $DISK)."
# -NoExit keeps the window open so the result stays readable; -Wait returns here when it is closed.
winps "Start-Process powershell -Verb RunAs -Wait -ArgumentList '-NoExit','-NoProfile','-ExecutionPolicy','Bypass','-File','\"$WRITER_WIN\"','-DiskNumber','$DISK','-Image','\"$IMAGE_WIN\"'"
echo ">>> The writer's window was closed."
if [[ $HAVE_PROPRIETARY == 1 ]]; then scripts/proprietary.sh copy || echo ">>> Proprietary files NOT copied: run scripts/proprietary.sh copy once the disk shows MIND CORE." >&2; fi
# Desktop pictures (wallpapers/README.md): not in the image either, copied to data/wallpapers/ and read back.
if [[ -z $IMAGE_FILE && -x scripts/disk_copy.sh ]] && compgen -G "wallpapers/*.jpg" > /dev/null; then
    ONLY='*.jpg' scripts/disk_copy.sh wallpapers data/wallpapers wallpapers || echo ">>> Desktop pictures NOT copied: run ONLY='*.jpg' scripts/disk_copy.sh wallpapers data/wallpapers wallpapers." >&2
fi
for flag in "${FLAGS[@]}"; do put_flag "$flag" || echo ">>> data/${flag%%:*} NOT put on the disk." >&2; done
if [[ $WIFIJOIN == 1 ]]; then put_wifijoin || echo ">>> The Wi-Fi network NOT put on the disk." >&2; fi
if [[ -n $LINK_BAUD ]]; then put_link "$LINK_BAUD" || echo ">>> link.conf NOT put on the disk." >&2; fi
if [[ $KEEP_WIFI == 1 ]]; then put_keystore || echo ">>> The remembered Wi-Fi networks NOT put on the disk." >&2; fi
