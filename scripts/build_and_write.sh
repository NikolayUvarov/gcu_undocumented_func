#!/usr/bin/env bash
# Pull a branch (or take one commit), build MIND Core, pack the USB image, then write it to a disk from an elevated
# Windows PowerShell: the maintainer's one step from WSL to a written USB disk (docs/write-disk.md, section 5).
# The clone it builds is the repository it lives in, also when it is started through a symbolic link from outside.
# Usage: scripts/build_and_write.sh [--commit <id> | --image <file>] [--slots] [--wifitest] [--wifi-ofdm[=<cores off>]] [--wifijoin] [--link[=<baud>]] [--link-run] [--link-dev] [--link-address=<n>] [--keep-wifi] [--keep-effector] [--keep-settings] [--mode=<modes>] <disk number>   |   --list
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
#   --link-address=<n>  as --link, with the word `address=<n>` (1 to 247) in link.conf: link then answers only frames
#                  to that address, so several machines share one RS-485 line (800-LNK-0026; `mindlink --address`,
#                  effector485's `addresses`). Without it, the machine must be alone on its line.
#   --mode=<modes> after the write, put data/mode.conf on the boot volume with the modes, one line: `maintenance`,
#                  `development`, or both comma-separated (`--mode=development,maintenance`). init reads it at start
#                  and runs the Effector agent's services in them (750-KRN-0143, docs/effector/README.md); `mode`
#                  in the shell changes it later. `link-console` beside them, with --link-dev, makes the link the
#                  owner's console (800-KRN-0155, 800-LNK-0019): only the disk's word turns it on or off.
#   --keep-wifi    keep the Wi-Fi networks the machine remembers across the write: before it, system/keystore/
#                  (wifi.kept, device.key, wifi.country) is saved from the disk's volume MIND CORE into
#                  ~/.config/mind/keystore (mode 600; a fresh disk uses what was saved last), and after it put back,
#                  with data/wifijoin.txt holding the network's name alone (the first line of ~/.config/mind/wifijoin,
#                  or KEEP_WIFI_NAME) and tests/hardware/wifijoin.msh as autorun.msh: the shell joins it at start, no
#                  passphrase on the disk. The saved files hold the network's key: never printed or committed.
#                  The network policy as changed on the machine (system/netpolicy/netpolicy.txt, `netpolicy add`) is
#                  kept too, in ~/.config/mind/netpolicy: put back as the new image's shipped policy with the same
#                  lines added and removed, so a rule like `netcheck <gateway> icmp` need not be typed again.
#   --keep-effector keep the Effector agent across the write (800-LNK-0023): before it, data/effector/ (its
#                  configuration, the owner's policy) and system/effector/ (its ID, journal, token and the answers
#                  still waiting for their ACK) are saved from the volume MIND CORE into
#                  ~/.config/mind/effector (700, files 600; a fresh disk uses what was saved last), and after it put
#                  back and read back. With --mode=maintenance the agent then comes up as the same endpoint, with no
#                  owner step. The files may hold the agent's token: never printed or committed.
#   --keep-settings keep everything the owner set on the machine (800-LNK-0024): --keep-wifi and --keep-effector,
#                  and the rest of data/ and system/: the time zone (data/clock.conf), the desktop (data/wm.conf and
#                  its picture), data/memory.conf, data/services.txt, data/mode.conf, installed apps, the owner's files.
#                  Saved into ~/.config/mind/settings (700/600) and put back first, so an option given now (--mode=
#                  and the others) still wins. Not kept: what the script puts itself (data/firmware, data/wallpapers,
#                  the test flags wifitest.txt, wifi-ofdm.txt, wifijoin.txt) and data/link (the link's scratch, old
#                  releases).
#                  With --keep-wifi, --keep-effector or --keep-settings, the old disk must be inserted before the write,
#                  not only when the writer asks: if it is not, the script asks for it and waits (OLD_DISK_WAIT, 600 s;
#                  Enter skips).
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
# The arguments as given, for a run of the new script when the pull below changes this one (800-LNK-0025).
ORIG_ARGS=("$@")
UPDATED="${BUILD_AND_WRITE_UPDATED:-}" RESTORE_BRANCH="${BUILD_AND_WRITE_RESTORE_BRANCH:-}"
export -n BUILD_AND_WRITE_UPDATED BUILD_AND_WRITE_RESTORE_BRANCH
# Only the copy itself is ever removed.
[[ ${BASH_SOURCE[0]} == "$COPY" && $COPY != "$SELF" ]] || { echo "build_and_write: not running from its copy ($COPY)" >&2; exit 1; }
trap 'rm -f -- "$COPY"' EXIT

REPO="${REPO:-$(git -C "$(dirname -- "$SELF")" rev-parse --show-toplevel)}"
BRANCH="${BRANCH:-fast-test}"
WRITER_WIN="$(wslpath -w "$REPO/05_write_usb_windows.ps1")"

# Runs PowerShell code passed as base64 UTF-16LE, so no quoting is lost between WSL and Windows.
winps() { powershell.exe -NoProfile -ExecutionPolicy Bypass -EncodedCommand "$(printf '%s' "$1" | iconv -t UTF-16LE | base64 -w0)"; }

COMMIT=""; FLAGS=(); MODES=""; SLOTS=0; WIFIJOIN=0; LINK_BAUD=; LINK_RUN=0; LINK_DEV=0; LINK_ADDRESS=; KEEP_WIFI=0; KEEP_EFFECTOR=0; KEEP_SETTINGS=0; IMAGE_FILE=
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
        --link-address=*) LINK_ADDRESS="${1#--link-address=}"
                          [[ "$LINK_ADDRESS" =~ ^[0-9]+$ ]] && (( 10#$LINK_ADDRESS >= 1 && 10#$LINK_ADDRESS <= 247 )) || { echo "--link-address=<n>: 1 to 247" >&2; exit 1; }
                          LINK_ADDRESS=$((10#$LINK_ADDRESS)); shift ;;
        --keep-wifi) KEEP_WIFI=1; shift ;;
        --keep-effector) KEEP_EFFECTOR=1; shift ;;
        --keep-settings) KEEP_SETTINGS=1; KEEP_WIFI=1; KEEP_EFFECTOR=1; shift ;;
        --mode=*) m="${1#--mode=}"; words=()
                  for w in ${m//,/ }; do
                      [[ $w == development || $w == maintenance || $w == link-console ]] || { echo "--mode=<modes>: development, maintenance, link-console, comma-separated: $w" >&2; exit 1; }
                      [[ " ${words[*]-} " == *" $w "* ]] || words+=("$w")
                  done
                  [[ ${#words[@]} -gt 0 ]] || { echo "--mode=<modes>: development, maintenance, link-console, comma-separated" >&2; exit 1; }
                  MODES=" ${words[*]} "; FLAGS+=("mode.conf:${words[*]}"); shift ;;
        *) break ;;
    esac
done
# init counts link-console only with `dev` in link.conf (800-KRN-0155).
[[ $MODES != *" link-console "* || $LINK_DEV == 1 ]] || { echo "--mode=link-console needs --link-dev: init leaves the word out without dev in link.conf" >&2; exit 1; }

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
    local conf="baud=$1"; [[ $LINK_RUN == 1 ]] && conf+=" run"; [[ $LINK_DEV == 1 ]] && conf+=" dev"; [[ -n $LINK_ADDRESS ]] && conf+=" address=$LINK_ADDRESS"
    winps "${script//__CONF__/$conf}" | tr -d '\r'
}

# The remembered Wi-Fi networks (keystore's files, 550-NET-0024), carried across a write through WSL's mount of the
# volume, so their bytes are on no command line; the mount is undone at once (the writer needs the volume unlocked).
KEEP_DIR="${KEEP_DIR:-$HOME/.config/mind/keystore}"
KEPT_FILES=(wifi.kept device.key wifi.country)
# The network policy as the owner changed it (108), with the shipped policy it was changed from.
POLICY_KEEP="${POLICY_KEEP:-$HOME/.config/mind/netpolicy}"
# The old disk's volume MIND CORE before the write, in OLD_LETTER. The disk is often still in its machine until the
# writer asks for it (800-LNK-0023), so the script asks for it now and waits up to OLD_DISK_WAIT seconds; Enter goes on
# without it. Called once, before the saves.
OLD_DISK_WAIT="${OLD_DISK_WAIT:-600}"
OLD_LETTER=""
find_old_volume() {
    # The disk's state: its volume MIND CORE's letter; `mounting` while that volume has none yet; `fresh` when the disk
    # holds no such volume (a new stick, another system); `absent` when there is no such disk.
    local ps="\$ProgressPreference = 'SilentlyContinue'; if (-not (Get-Disk -Number $DISK -ErrorAction SilentlyContinue)) { 'absent' } else { \$m = Get-Partition -DiskNumber $DISK -ErrorAction SilentlyContinue | Get-Volume -ErrorAction SilentlyContinue | Where-Object { \$_.FileSystemLabel -eq 'MIND CORE' } | Select-Object -First 1; if (\$m -and \$m.DriveLetter) { \$m.DriveLetter } elseif (\$m) { 'mounting' } else { 'fresh' } }"
    local state fresh=0
    state=$(winps "$ps" | tr -d '\r\n')
    if [[ ! $state =~ ^[A-Z]$ && $state != fresh ]]; then
        echo ">>> Insert the disk to be written (disk $DISK) now: what it keeps (--keep-wifi, --keep-effector, --keep-settings) is saved from it before the write. Waiting up to $((OLD_DISK_WAIT / 60)) min; Enter goes on without it."
    fi
    local end=$((SECONDS + OLD_DISK_WAIT)) tty=0
    # Opened only when it opens: a failed exec redirection would end the script.
    if (: < /dev/tty) 2> /dev/null; then exec 3< /dev/tty; tty=1; fi
    while [[ ! $state =~ ^[A-Z]$ ]] && (( SECONDS < end )); do
        # A disk with no volume MIND CORE, seen twice 3 s apart (Windows names a volume a moment after the disk), has
        # nothing to save: the write goes on at once.
        if [[ $state == fresh ]]; then
            if (( ++fresh >= 2 )); then echo ">>> Disk $DISK holds no volume MIND CORE (a new disk): nothing is saved from it; what was saved before is put back."; break; fi
        else fresh=0; fi
        if (( tty )); then
            if read -r -t 3 _ <&3; then echo ">>> Going on without the old disk: what was saved before is put back."; break; fi
        else sleep 3; fi
        state=$(winps "$ps" | tr -d '\r\n')
        [[ $state =~ ^[A-Z]$ ]] && echo ">>> Disk $DISK is here, its volume MIND CORE at $state:"
    done
    (( tty )) && exec 3<&-
    OLD_LETTER=""
    [[ $state =~ ^[A-Z]$ ]] && OLD_LETTER=$state
    return 0
}
save_keystore() {
    local letter dir n=0
    letter=$OLD_LETTER
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

# The Effector agent's files (800-LNK-0023), carried across a write as keystore's are: the owner's configuration and
# policy in data/effector/, the agent's own state in system/effector/. Only the owner's shell writes them on the machine.
EFFECTOR_KEEP="${EFFECTOR_KEEP:-$HOME/.config/mind/effector}"
# Copies the volume at $1's agent files into $EFFECTOR_KEEP, whole or not at all; echoes how many files.
keep_effector_from() {
    local vol=$1 next="$EFFECTOR_KEEP.next" n
    [[ -d $vol/data/effector || -d $vol/system/effector ]] || return 1
    rm -rf "$next"; (umask 077; mkdir -p "$next/data" "$next/system")
    [[ -d $vol/data/effector ]] && (umask 077; cp -r "$vol/data/effector/." "$next/data/")
    [[ -d $vol/system/effector ]] && (umask 077; cp -r "$vol/system/effector/." "$next/system/")
    find "$next" -type d -exec chmod 700 {} + && find "$next" -type f -exec chmod 600 {} +
    n=$(find "$next" -type f | wc -l)
    rm -rf "${EFFECTOR_KEEP:?}" && mv "$next" "$EFFECTOR_KEEP" && echo "$n"
}
# Puts $EFFECTOR_KEEP's files on the volume at $1 and reads them back.
put_effector_to() {
    local vol=$1
    mkdir -p "$vol/data/effector" "$vol/system/effector"
    cp -r "$EFFECTOR_KEEP/data/." "$vol/data/effector/" && cp -r "$EFFECTOR_KEEP/system/." "$vol/system/effector/" && sync || return 1
    local f
    while IFS= read -r f; do cmp -s "$EFFECTOR_KEEP/data/$f" "$vol/data/effector/$f" || return 1; done < <(cd "$EFFECTOR_KEEP/data" && find . -type f)
    while IFS= read -r f; do cmp -s "$EFFECTOR_KEEP/system/$f" "$vol/system/effector/$f" || return 1; done < <(cd "$EFFECTOR_KEEP/system" && find . -type f)
}
save_effector() {
    local letter dir n
    letter=$OLD_LETTER
    if [[ ! $letter =~ ^[A-Z]$ ]]; then
        if [[ -d $EFFECTOR_KEEP ]]; then echo "keep-effector: disk $DISK not inserted; what was saved before will be put back"
        else echo ">>> keep-effector: disk $DISK not inserted and nothing saved before: the agent's files will NOT be kept" >&2; fi
        return 0
    fi
    dir=$(mktemp -d)
    sudo mount -t drvfs "$letter:" "$dir" -o "uid=$(id -u),gid=$(id -g)" || { rmdir "$dir"; return 1; }
    if n=$(keep_effector_from "$dir"); then echo "keep-effector: $n file(s) of $letter:\\data\\effector and \\system\\effector saved in $EFFECTOR_KEEP"
    else echo "keep-effector: $letter: holds no Effector agent; what was saved before will be put back"; fi
    sudo umount "$dir"; rmdir "$dir"
}
put_effector() {
    [[ -d $EFFECTOR_KEEP/data || -d $EFFECTOR_KEEP/system ]] || { echo "keep-effector: nothing saved in $EFFECTOR_KEEP: put the agent's files in once (the owner's start.msh), then write with --keep-effector" >&2; return 1; }
    local letter dir status=0
    letter=$(winps "${PS_LETTER//__MANIFEST__/$(sha256sum "$MANIFEST_FILE" | cut -d' ' -f1)}" | tr -d '\r\n')
    [[ $letter =~ ^[A-Z]$ ]] || { echo "keep-effector: no volume MIND CORE holding this build (its MANIFEST) within 60 s" >&2; return 1; }
    dir=$(mktemp -d)
    sudo mount -t drvfs "$letter:" "$dir" -o "uid=$(id -u),gid=$(id -g)" || { rmdir "$dir"; return 1; }
    if put_effector_to "$dir"; then echo "keep-effector: the Effector agent's $(find "$EFFECTOR_KEEP" -type f | wc -l) file(s) put back on $letter:, read back"
    else echo "keep-effector: the Effector agent's files NOT put back" >&2; status=1; fi
    sudo umount "$dir"; rmdir "$dir"
    winps "\$ProgressPreference = 'SilentlyContinue'; Write-VolumeCache -DriveLetter $letter" > /dev/null || true
    return $status
}

# Everything else the owner set (800-LNK-0024): data/ and system/ but for what other options carry with their own
# care (keystore, the network policy's merge, the agent) and what the script puts itself.
SETTINGS_KEEP="${SETTINGS_KEEP:-$HOME/.config/mind/settings}"
SETTINGS_SKIP_DATA=(effector firmware wallpapers link wifitest.txt wifi-ofdm.txt wifijoin.txt)
SETTINGS_SKIP_SYSTEM=(keystore netpolicy effector)
# Copies the volume at $1's settings into $SETTINGS_KEEP, whole or not at all; echoes how many files.
keep_settings_from() {
    local vol=$1 next="$SETTINGS_KEEP.next" sub x args
    [[ -d $vol/data || -d $vol/system ]] || return 1
    rm -rf "$next"; (umask 077; mkdir -p "$next/data" "$next/system")
    for sub in data system; do
        [[ -d $vol/$sub ]] || continue
        args=()
        if [[ $sub == data ]]; then for x in "${SETTINGS_SKIP_DATA[@]}"; do args+=("--exclude=./$x"); done
        else for x in "${SETTINGS_SKIP_SYSTEM[@]}"; do args+=("--exclude=./$x"); done; fi
        (cd "$vol/$sub" && tar cf - "${args[@]}" .) | (umask 077; cd "$next/$sub" && tar xf - --no-same-owner) || { rm -rf "$next"; return 1; }
    done
    find "$next" -type d -exec chmod 700 {} + && find "$next" -type f -exec chmod 600 {} +
    rm -rf "${SETTINGS_KEEP:?}" && mv "$next" "$SETTINGS_KEEP" && find "$SETTINGS_KEEP" -type f | wc -l
}
# Puts $SETTINGS_KEEP's files on the volume at $1 and reads each back.
put_settings_to() {
    local vol=$1 sub f
    for sub in data system; do
        mkdir -p "$vol/$sub" && cp -r "$SETTINGS_KEEP/$sub/." "$vol/$sub/" || return 1
    done
    sync
    while IFS= read -r f; do cmp -s "$SETTINGS_KEEP/$f" "$vol/$f" || { echo "keep-settings: $f differs on the disk" >&2; return 1; }; done < <(cd "$SETTINGS_KEEP" && find . -type f)
}
save_settings() {
    local letter=$OLD_LETTER dir n
    if [[ ! $letter =~ ^[A-Z]$ ]]; then
        if [[ -d $SETTINGS_KEEP ]]; then echo "keep-settings: disk $DISK not inserted; what was saved before will be put back"
        else echo ">>> keep-settings: disk $DISK not inserted and nothing saved before: the settings will NOT be kept" >&2; fi
        return 0
    fi
    dir=$(mktemp -d)
    sudo mount -t drvfs "$letter:" "$dir" -o "uid=$(id -u),gid=$(id -g)" || { rmdir "$dir"; return 1; }
    if n=$(keep_settings_from "$dir"); then
        echo "keep-settings: $n file(s) of $letter:\\data and \\system saved in $SETTINGS_KEEP ($(du -sh "$SETTINGS_KEEP" | cut -f1)): $(ls "$SETTINGS_KEEP/data" | tr '\n' ' ')"
    else echo "keep-settings: $letter: holds no settings; what was saved before will be put back"; fi
    sudo umount "$dir"; rmdir "$dir"
}
put_settings() {
    [[ -d $SETTINGS_KEEP ]] || { echo "keep-settings: nothing saved in $SETTINGS_KEEP" >&2; return 1; }
    local letter dir status=0
    letter=$(winps "${PS_LETTER//__MANIFEST__/$(sha256sum "$MANIFEST_FILE" | cut -d' ' -f1)}" | tr -d '\r\n')
    [[ $letter =~ ^[A-Z]$ ]] || { echo "keep-settings: no volume MIND CORE holding this build (its MANIFEST) within 60 s" >&2; return 1; }
    dir=$(mktemp -d)
    sudo mount -t drvfs "$letter:" "$dir" -o "uid=$(id -u),gid=$(id -g)" || { rmdir "$dir"; return 1; }
    if put_settings_to "$dir"; then echo "keep-settings: $(find "$SETTINGS_KEEP" -type f | wc -l) file(s) put back on $letter:, read back"
    else echo "keep-settings: the settings NOT all put back" >&2; status=1; fi
    sudo umount "$dir"; rmdir "$dir"
    winps "\$ProgressPreference = 'SilentlyContinue'; Write-VolumeCache -DriveLetter $letter" > /dev/null || true
    return $status
}

if [[ "${1:-}" == "--list" ]]; then
    winps "\$ProgressPreference = 'SilentlyContinue'; & '$WRITER_WIN' -List" | tr -d '\r'
    exit 0
fi
[[ ($LINK_RUN == 1 || $SLOTS == 1 || -n $LINK_ADDRESS) && -z $LINK_BAUD ]] && LINK_BAUD=460800
[[ $SLOTS == 1 && -n $IMAGE_FILE ]] && { echo "--slots builds the image; it does not go with --image" >&2; exit 1; }
DISK="${1:?usage: build_and_write.sh [--commit <id>] [--slots] [--wifitest] [--wifi-ofdm[=<cores off>]] [--wifijoin] [--link[=<baud>]] [--link-run] [--link-dev] [--link-address=<n>] [--keep-wifi] [--keep-effector] [--keep-settings] [--mode=<modes>] <disk number> | --list}"
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
if [[ -z $UPDATED ]]; then
    git fetch origin
    if [[ -n "$COMMIT" ]]; then
        git checkout --detach "$COMMIT"
        trap 'git -C "$REPO" checkout -q "$BRANCH" || echo ">>> The clone stays at $COMMIT: git checkout $BRANCH there." >&2; rm -f -- "$COPY"' EXIT
    else
        git checkout "$BRANCH"
        git pull --ff-only
    fi
    # The checkout or the pull may have changed this script: the new one runs, with the same arguments, before
    # anything is saved or written (the maintainer's word, 2026-10-24). exec runs no EXIT trap, so the copy goes
    # first, and with --commit the new run gives the branch back itself.
    if [[ -f $SELF ]] && ! cmp -s -- "$SELF" "$COPY"; then
        echo ">>> build_and_write.sh changed with the $([[ -n $COMMIT ]] && echo checkout || echo pull): running the new one"
        rm -f -- "$COPY"
        BUILD_AND_WRITE_UPDATED=1 BUILD_AND_WRITE_RESTORE_BRANCH="$([[ -n $COMMIT ]] && echo "$BRANCH")" exec bash "$SELF" "${ORIG_ARGS[@]}"
    fi
elif [[ -n $RESTORE_BRANCH ]]; then
    trap 'git -C "$REPO" checkout -q "$RESTORE_BRANCH" || echo ">>> The clone stays at $COMMIT: git checkout $RESTORE_BRANCH there." >&2; rm -f -- "$COPY"' EXIT
fi
[[ -n "$COMMIT" ]] && echo ">>> Building commit $(git log -1 --format='%h %ad %s' --date=short)"
echo ">>> build_and_write.sh $(git log -1 --format=%h -- scripts/build_and_write.sh) runs$([[ -n $UPDATED ]] && { [[ -n $RESTORE_BRANCH ]] && echo ", the commit's own" || echo ', the version the pull brought'; })"
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

if [[ $KEEP_WIFI == 1 || $KEEP_EFFECTOR == 1 ]]; then find_old_volume; fi
if [[ $KEEP_WIFI == 1 ]]; then save_keystore || echo ">>> The remembered Wi-Fi networks NOT saved from disk $DISK." >&2; fi
if [[ $KEEP_EFFECTOR == 1 ]]; then save_effector || echo ">>> The Effector agent's files NOT saved from disk $DISK." >&2; fi
if [[ $KEEP_SETTINGS == 1 ]]; then save_settings || echo ">>> The settings NOT saved from disk $DISK." >&2; fi
echo ">>> Opening an elevated PowerShell to write $IMAGE_WIN to disk $DISK (UAC, then type ERASE $DISK)."
# -NoExit keeps the window open so the result stays readable; -Wait returns here when it is closed.
winps "Start-Process powershell -Verb RunAs -Wait -ArgumentList '-NoExit','-NoProfile','-ExecutionPolicy','Bypass','-File','\"$WRITER_WIN\"','-DiskNumber','$DISK','-Image','\"$IMAGE_WIN\"'"
echo ">>> The writer's window was closed."
# The owner's settings first: what the script puts after them (firmware, pictures, --mode= and the others) wins.
if [[ $KEEP_SETTINGS == 1 ]]; then put_settings || echo ">>> The settings NOT all put on the disk." >&2; fi
if [[ $HAVE_PROPRIETARY == 1 ]]; then scripts/proprietary.sh copy || echo ">>> Proprietary files NOT copied: run scripts/proprietary.sh copy once the disk shows MIND CORE." >&2; fi
# Desktop pictures (wallpapers/README.md): not in the image either, copied to data/wallpapers/ and read back.
if [[ -z $IMAGE_FILE && -x scripts/disk_copy.sh ]] && compgen -G "wallpapers/*.jpg" > /dev/null; then
    ONLY='*.jpg' scripts/disk_copy.sh wallpapers data/wallpapers wallpapers || echo ">>> Desktop pictures NOT copied: run ONLY='*.jpg' scripts/disk_copy.sh wallpapers data/wallpapers wallpapers." >&2
fi
for flag in "${FLAGS[@]}"; do put_flag "$flag" || echo ">>> data/${flag%%:*} NOT put on the disk." >&2; done
if [[ $WIFIJOIN == 1 ]]; then put_wifijoin || echo ">>> The Wi-Fi network NOT put on the disk." >&2; fi
if [[ -n $LINK_BAUD ]]; then put_link "$LINK_BAUD" || echo ">>> link.conf NOT put on the disk." >&2; fi
if [[ $KEEP_WIFI == 1 ]]; then put_keystore || echo ">>> The remembered Wi-Fi networks NOT put on the disk." >&2; fi
if [[ $KEEP_EFFECTOR == 1 ]]; then put_effector || echo ">>> The Effector agent's files NOT put on the disk." >&2; fi
