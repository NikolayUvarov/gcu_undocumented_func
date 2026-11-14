#!/usr/bin/env bash
# Pull a branch (or take one commit), build MIND Core, pack the USB image, then write it to a disk from an elevated
# Windows PowerShell: the maintainer's one step from WSL to a written USB disk (docs/write-disk.md, section 5).
# The clone it builds is the repository it lives in, also when it is started through a symbolic link from outside.
# Usage: scripts/build_and_write.sh [--commit <id> | --image <file>] [--slots] [--wifitest] [--wifi-ofdm[=<cores off>]] [--wifijoin] [--link[=<baud>]] [--link-run] [--link-dev] [--link-address=<n>] [--keep-wifi] [--keep-effector] [--keep-settings] [--machine <id>] [--old-disk <n>] [--new-device-key] [--mode=<modes>] <disk number>   |   --list   |   --list-machines
#   --commit <id>  build that commit (a short or full hash, a tag) instead of the branch's latest; the clone returns
#                  to the branch when the script ends, so this script stays in it.
#   --image <file> write that image instead of building one (a trial built elsewhere); the image's MANIFEST must lie
#                  beside it, so the steps after the write find its volume. Proprietary files, wallpapers and
#                  mac/startup-disk.sh are not copied then.
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
#                  `remote-update` (751), with development or maintenance, lets a developer's release sent through
#                  Effector be installed; `link-remote-update`, with link-console, lets the link's console switch it.
#   --keep-wifi    keep the Wi-Fi networks the machine remembers, and its device key, across the write: before it,
#                  system/keystore/ (wifi.kept, device.key, wifi.country, and device.sealed or device.locked where a TPM
#                  or the owner's passphrase seals the key: they open on that machine alone) is saved from the disk's
#                  volume MIND CORE into the machine's keystore/ (below; mode 600), and after it put back, with data/wifijoin.txt holding the network's
#                  name alone (the first line of ~/.config/mind/wifijoin, or KEEP_WIFI_NAME) and
#                  tests/hardware/wifijoin.msh as autorun.msh: the shell joins it at start, no passphrase on the disk.
#                  The saved files hold the network's key: never printed or committed.
#                  The network policy as changed on the machine (system/netpolicy/netpolicy.txt, `netpolicy add`) is
#                  kept too, in the machine's netpolicy/: put back as the new image's shipped policy with the same
#                  lines added and removed, so a rule like `netcheck <gateway> icmp` need not be typed again.
#   --keep-effector keep the Effector agent across the write (800-LNK-0023): before it, data/effector/ (its
#                  configuration, the owner's policy) and system/effector/ (its ID, journal, token and the answers
#                  still waiting for their ACK) are saved from the volume MIND CORE into the machine's effector/
#                  (700, files 600), and after it put back and read back. With --mode=maintenance the agent then comes
#                  up as the same endpoint, with no owner step. The files may hold the agent's token: never printed or
#                  committed. A missing or empty part or file of the old disk keeps what was saved before for that
#                  machine, and the copy it replaces stays beside it as effector.prev (800-LNK-0036); the same holds for
#                  --keep-wifi and --keep-settings.
#   --keep-settings keep everything the owner set on the machine (800-LNK-0024): --keep-wifi and --keep-effector,
#                  and the rest of data/ and system/: the time zone (data/clock.conf), the desktop (data/wm.conf and
#                  its picture), data/memory.conf, data/services.txt, data/mode.conf, installed apps, the owner's files.
#                  Saved into the machine's settings/ (700/600) and put back first, so an option given now (--mode=
#                  and the others) still wins. Not kept: what the script puts itself (data/firmware, data/wallpapers,
#                  the test flags wifitest.txt, wifi-ofdm.txt, wifijoin.txt) and data/link (the link's scratch, old
#                  releases).
#                  Each machine's are kept apart (801-LNK-0046), in ~/.config/mind/machines/<id>/ (700): keystore/,
#                  netpolicy/, effector/, settings/ and machine.txt (the ID, the host name, the agent's machine print,
#                  the date saved, the old disk's commit; no secret). <id> is the machine's Effector agent ID as the
#                  old disk records it (system/effector/agent_id, else data/effector/agent_id), else host-<its name>
#                  (data/machine/name, else effector.conf's hostname); never a secret. The old disk's files go into
#                  its own machine's directory; after the write, the settings of the machine --machine names, else of
#                  the old disk just read, go on the new disk. With neither, the write is refused before the writer
#                  opens: never the machine saved last. A disk that names no machine is not read. The one store of
#                  before (~/.config/mind/settings, effector, keystore, netpolicy) is left untouched and unused.
#                  One device key on one machine: machine.txt records the key's public label (MIND and 8 hex digits, as
#                  keystore logs it; from device.key, or from the old disk's newest boot log for a sealed key), and a
#                  write whose machine's kept key another kept machine holds too is refused before the writer opens,
#                  both named. The key itself is never printed.
#                  With --keep-wifi, --keep-effector or --keep-settings, the old disk must be inserted before the write,
#                  not only when the writer asks: if it is not, the script asks for it and waits (OLD_DISK_WAIT, 600 s;
#                  Enter skips).
#   --machine <id> with the --keep options: put the settings kept for that machine (--list-machines) on the disk. An
#                  old disk that names another machine is then not read, and that machine's kept files stay as they are.
#   --old-disk <n> with the --keep options: the old disk is another device than the disk written (a new stick). It is
#                  read first as disk <n>; then the script asks to take it out and insert the new one as <disk number>,
#                  and waits until a device other than the old one is there (Enter goes on as it is).
#   --new-device-key with --keep-wifi: the machine gets a new device key. Its kept key and the networks wrapped under it
#                  are moved aside in its keystore/replaced-<date>/ (never removed) and not put on the disk, so keystore
#                  makes a new key there; join the networks again by hand, or with --wifijoin. For a machine whose key
#                  another machine holds too (above).
#   --list-machines the machines whose settings are kept: the ID, the host name, the date saved, the parts kept and the
#                  device key's public label; no secret.
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

# The pull comes before the options are read (801-LNK-0046): the new script reads them, so an option this copy does not
# know yet still works. --commit is looked for here; --image, --list and --list-machines fetch nothing.
EARLY_COMMIT=""; EARLY_SKIP=0
for ((i = 1; i <= $#; i++)); do
    case "${!i}" in
        --commit) j=$((i + 1)); EARLY_COMMIT="${!j:-}" ;;
        --image|--list|--list-machines) EARLY_SKIP=1 ;;
    esac
done
if [[ -z $UPDATED && $EARLY_SKIP == 0 ]]; then
    git -C "$REPO" fetch origin
    if [[ -n $EARLY_COMMIT ]]; then
        git -C "$REPO" checkout --detach "$EARLY_COMMIT"
        trap 'git -C "$REPO" checkout -q "$BRANCH" || echo ">>> The clone stays at $EARLY_COMMIT: git checkout $BRANCH there." >&2; rm -f -- "$COPY"' EXIT
    else
        git -C "$REPO" checkout "$BRANCH"
        git -C "$REPO" pull --ff-only
    fi
    # The checkout or the pull may have changed this script: the new one runs, with the same arguments, before
    # anything is saved or written (the maintainer's word, 2026-10-24). exec runs no EXIT trap, so the copy goes
    # first, and with --commit the new run gives the branch back itself.
    if [[ -f $SELF ]] && ! cmp -s -- "$SELF" "$COPY"; then
        echo ">>> build_and_write.sh changed with the $([[ -n $EARLY_COMMIT ]] && echo checkout || echo pull): running the new one"
        rm -f -- "$COPY"
        BUILD_AND_WRITE_UPDATED=1 BUILD_AND_WRITE_RESTORE_BRANCH="$([[ -n $EARLY_COMMIT ]] && echo "$BRANCH")" exec bash "$SELF" "${ORIG_ARGS[@]}"
    fi
elif [[ -n $RESTORE_BRANCH ]]; then
    trap 'git -C "$REPO" checkout -q "$RESTORE_BRANCH" || echo ">>> The clone stays at the commit: git checkout $RESTORE_BRANCH there." >&2; rm -f -- "$COPY"' EXIT
fi

MODE_WORDS="development maintenance link-console remote-update link-remote-update"
COMMIT=""; FLAGS=(); MODES=""; SLOTS=0; WIFIJOIN=0; LINK_BAUD=; LINK_RUN=0; LINK_DEV=0; LINK_ADDRESS=; KEEP_WIFI=0; KEEP_EFFECTOR=0; KEEP_SETTINGS=0; IMAGE_FILE=
MACHINE=""; OLD_DISK=""; OLD_GIVEN=0; NEW_KEY=0
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
        --machine) MACHINE="${2:?usage: $0 --machine <id> <disk number>}"; shift 2
                   [[ $MACHINE =~ ^[A-Za-z0-9_-]{1,68}$ ]] || { echo "--machine <id>: 1 to 68 of A-Z a-z 0-9 _ -, as --list-machines shows it: $MACHINE" >&2; exit 1; } ;;
        --old-disk) OLD_DISK="${2:?usage: $0 --old-disk <n> <disk number>}"; OLD_GIVEN=1; shift 2
                    [[ $OLD_DISK =~ ^[0-9]+$ ]] || { echo "--old-disk <n>: the old disk's number (--list), digits: $OLD_DISK" >&2; exit 1; } ;;
        --new-device-key) NEW_KEY=1; shift ;;
        --mode=*) m="${1#--mode=}"; words=()
                  for w in ${m//,/ }; do
                      [[ " $MODE_WORDS " == *" $w "* ]] || { echo "--mode=<modes>: ${MODE_WORDS// /, }, comma-separated: $w" >&2; exit 1; }
                      [[ " ${words[*]-} " == *" $w "* ]] || words+=("$w")
                  done
                  [[ ${#words[@]} -gt 0 ]] || { echo "--mode=<modes>: ${MODE_WORDS// /, }, comma-separated" >&2; exit 1; }
                  MODES=" ${words[*]} "; FLAGS+=("mode.conf:${words[*]}"); shift ;;
        --list|--list-machines) break ;;
        --*) echo "build_and_write.sh: unknown option $1 (the usage is at the top of scripts/build_and_write.sh)" >&2; exit 1 ;;
        *) break ;;
    esac
done
# init counts link-console only with `dev` in link.conf (800-KRN-0155).
[[ $MODES != *" link-console "* || $LINK_DEV == 1 ]] || { echo "--mode=link-console needs --link-dev: init leaves the word out without dev in link.conf" >&2; exit 1; }
# init counts remote-update only in the development or maintenance mode, link-remote-update only with link-console (751).
[[ $MODES != *" remote-update "* || $MODES == *" development "* || $MODES == *" maintenance "* ]] || { echo "--mode=remote-update needs development or maintenance: init counts it only in them (751)" >&2; exit 1; }
[[ $MODES != *" link-remote-update "* || $MODES == *" link-console "* ]] || { echo "--mode=link-remote-update needs link-console: it lets the link's console switch remote-update (751)" >&2; exit 1; }
# --machine and --old-disk say whose kept settings go on the disk and where they are read from (801-LNK-0046).
[[ ( -z $MACHINE && $OLD_GIVEN == 0 ) || $KEEP_WIFI == 1 || $KEEP_EFFECTOR == 1 ]] || { echo "--machine and --old-disk go with --keep-settings, --keep-wifi or --keep-effector: they say whose kept settings go on the disk" >&2; exit 1; }
[[ $NEW_KEY == 0 || $KEEP_WIFI == 1 ]] || { echo "--new-device-key goes with --keep-wifi or --keep-settings: it sets the machine's kept device key aside" >&2; exit 1; }

# Request files (data/NAME, holding TEXT) put on the volume MIND CORE that holds this build's MANIFEST (at its root, or in
# MIND/A with --slots), flushed and read back.
read -r -d '' PS_FLAG <<'PS' || true
$ErrorActionPreference = 'Stop'; $ProgressPreference = 'SilentlyContinue'; trap { Write-Output "flag: $_"; exit 1 }
for ($i = 0; $i -lt 30; $i++) {
    $v = Get-Volume | Where-Object { $_.FileSystemLabel -eq 'MIND CORE' -and $_.DriveLetter } | Where-Object { $l = $_.DriveLetter; @("$($l):\MANIFEST", "$($l):\MIND\A\MANIFEST" | Where-Object { (Test-Path $_) -and (Get-FileHash $_).Hash -eq '__MANIFEST__' }).Count } | Select-Object -First 1
    if ($v) { break }
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
    $v = Get-Volume | Where-Object { $_.FileSystemLabel -eq 'MIND CORE' -and $_.DriveLetter } | Where-Object { $l = $_.DriveLetter; @("$($l):\MANIFEST", "$($l):\MIND\A\MANIFEST" | Where-Object { (Test-Path $_) -and (Get-FileHash $_).Hash -eq '__MANIFEST__' }).Count } | Select-Object -First 1
    if ($v) { Write-Output $v.DriveLetter; exit 0 }
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
    $v = Get-Volume | Where-Object { $_.FileSystemLabel -eq 'MIND CORE' -and $_.DriveLetter } | Where-Object { $l = $_.DriveLetter; @("$($l):\MANIFEST", "$($l):\MIND\A\MANIFEST" | Where-Object { (Test-Path $_) -and (Get-FileHash $_).Hash -eq '__MANIFEST__' }).Count } | Select-Object -First 1
    if ($v) { break }
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

# A damaged or half-written old disk never erases what was saved (800-LNK-0036): a part or file of it that is missing
# or empty keeps the one saved before, and a saved copy that is replaced stays beside it as <dir>.prev.
has_content() { [[ -d $1 && -n $(find "$1" -type f -size +0c -print -quit) ]]; }
rotate_kept() {
    local next=$1 keep=$2
    rm -rf "${keep:?}.prev"; if [[ -e $keep ]]; then mv "$keep" "$keep.prev" || return 1; fi
    mv "$next" "$keep"
}
# File $1 kept as $2 when it holds something, the saved one staying as $2.prev; an empty or missing $1 keeps $2.
keep_file() {
    local from=$1 to=$2
    if [[ -s $from ]]; then
        [[ -s $to ]] && (umask 077; cp "$to" "$to.prev")
        (umask 077; cp "$from" "$to") && chmod 600 "$to"
    else
        [[ -f $from && -s $to ]] && echo "keep: ${from##*/} on the disk is empty: the copy saved before is kept" >&2
        return 1
    fi
}
# The empty files under $1 refilled from the same files under $2 where those hold something.
refill_empty() {
    local f
    [[ -d $2 ]] || return 0
    while IFS= read -r f; do [[ -s $2/$f ]] && cp "$2/$f" "$1/$f"; done < <(cd "$1" && find . -type f -size 0c)
    return 0
}

# Each machine's kept files apart, by the identity its old disk records (801-LNK-0046): never another machine's, never
# the one read last. use_machine points the keep and put functions below at one machine's directory.
MACHINES_KEEP="${MACHINES_KEEP:-$HOME/.config/mind/machines}"
MACHINE_DIR="" KEEP_DIR="" POLICY_KEEP="" EFFECTOR_KEEP="" SETTINGS_KEEP=""
use_machine() {
    MACHINE=$1 MACHINE_DIR="$MACHINES_KEEP/$1"
    KEEP_DIR="$MACHINE_DIR/keystore" POLICY_KEEP="$MACHINE_DIR/netpolicy" EFFECTOR_KEEP="$MACHINE_DIR/effector" SETTINGS_KEEP="$MACHINE_DIR/settings"
}
# The machine's name the volume at $1 records: data/machine/name (000-KRN-0180), else the hostname line of the agent's
# data/effector/effector.conf; nothing when neither holds one.
machine_name_of() {
    local name
    name=$(head -c 80 "$1/data/machine/name" 2> /dev/null | head -1 | tr -d '\r') || true
    if [[ $name =~ ^[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?$ ]]; then echo "$name"; return 0; fi
    name=$(tr -d '\r' 2> /dev/null < "$1/data/effector/effector.conf" | awk '$1 == "hostname" && NF == 2 { print $2; exit }') || true
    if [[ $name =~ ^[A-Za-z0-9_-]{1,63}$ ]]; then echo "$name"; fi
    return 0
}
# The machine the volume at $1 names: its Effector agent's ID (system/effector/agent_id, else the owner's
# data/effector/agent_id; 750-EFF-0017), else host-<its name>; fails when it names neither. The device key has no public
# part on the disk (keystore/src/stored.rs holds the seed alone), so it never names the machine: no secret is read.
machine_id_of() {
    local f id
    for f in "$1/system/effector/agent_id" "$1/data/effector/agent_id"; do
        id=$(head -c 80 "$f" 2> /dev/null | head -1 | tr -d '\r') || true
        if [[ $id =~ ^[A-Za-z0-9_-]{1,64}$ ]]; then echo "$id"; return 0; fi
    done
    id=$(machine_name_of "$1")
    if [[ -n $id && $id != mindcore ]]; then echo "host-$id"; return 0; fi
    return 1
}
# The commit the volume at $1 was built from: its MANIFEST's, else each slot's (A:…, B:…); nothing when unknown.
disk_commit() {
    local m c out=()
    for m in "" MIND/A/ MIND/B/; do
        c=$(tr -d '\r' 2> /dev/null < "$1/${m}MANIFEST" | awk '$1 == "commit" && NF == 2 { print substr($2, 1, 12); exit }') || true
        [[ $c =~ ^[0-9a-z]+$ ]] || continue
        if [[ -z $m ]]; then echo "$c"; return 0; fi
        m=${m%/}; out+=("${m#MIND/}:$c")
    done
    echo "${out[*]-}"
}
# The device key's public label, MIND and the first four bytes of its Ed25519 public key in hex, as keystore logs it
# (keystore/src/certificate.rs), from a device.key at $1 (MINDKEY1, the seed, their SHA-256): the seed goes to openssl
# through a pipe alone, never into a file, an argument or the output. Fails for anything else, a sealed key among them.
key_label() {
    local f=$1 pub
    [[ -f $f && $(stat -c %s "$f") == 72 && $(head -c 8 "$f" | tr -d '\0') == MINDKEY1 ]] || return 1
    [[ $(head -c 40 "$f" | sha256sum | cut -c1-64) == $(tail -c 32 "$f" | od -An -v -tx1 | tr -d ' \n') ]] || return 1
    # A PKCS#8 Ed25519 private key is this prefix and the seed (RFC 8410).
    pub=$({ printf '\x30\x2e\x02\x01\x00\x30\x05\x06\x03\x2b\x65\x70\x04\x22\x04\x20'; head -c 40 "$f" | tail -c 32; } | openssl pkey -inform DER -pubout -outform DER 2> /dev/null | tail -c 32 | head -c 4 | od -An -v -tx1 | tr -d ' \n' | tr a-f A-F) || return 1
    [[ $pub =~ ^[0-9A-F]{8}$ ]] || return 1
    echo "MIND $pub"
}
# The label the newest boot log (BOOTNNNN.LOG) of the log volume at $1 names in keystore's public line
# `[KEYSTORE] DEVICE KEY READY: MIND …`: the label of a key that a TPM or a passphrase seals, which no file holds.
log_label() {
    local f l
    while IFS= read -r f; do
        l=$(grep -ao 'DEVICE KEY READY: MIND [0-9A-F]\{8\}' "$1/$f" 2> /dev/null | tail -1) || true
        if [[ -n $l ]]; then echo "${l#DEVICE KEY READY: }"; return 0; fi
    done < <(ls -1 "$1" 2> /dev/null | grep -i '^BOOT[0-9]*\.LOG$' | sort -r | head -20)
    return 1
}
# The old disk's log volume (MIND LOG, 211-PRT-0006) on disk $1: its letter, or nothing.
log_letter() { winps "\$ProgressPreference = 'SilentlyContinue'; Get-Partition -DiskNumber $1 -ErrorAction SilentlyContinue | Get-Volume -ErrorAction SilentlyContinue | Where-Object { \$_.FileSystemLabel -eq 'MIND LOG' -and \$_.DriveLetter } | Select-Object -First 1 -ExpandProperty DriveLetter" | tr -d '\r\n'; }
# The label of the old disk's sealed key, from its log volume, mounted for the read alone.
old_log_label() {
    local letter dir label=""
    letter=$(log_letter "$OLD_DISK") || letter=""
    [[ $letter =~ ^[A-Z]$ ]] || return 1
    dir=$(mktemp -d)
    sudo mount -t drvfs "$letter:" "$dir" -o "uid=$(id -u),gid=$(id -g)" || { rmdir "$dir"; return 1; }
    label=$(log_label "$dir") || label=""
    sudo umount "$dir"; rmdir "$dir"
    [[ -n $label ]] && echo "$label"
}
# machine.txt in MACHINE's directory, from the volume at $1: what names the machine and when it was saved, and the
# device key's public label (DISK_KEY_LABEL, else the one recorded before); no secret.
DISK_KEY_LABEL=""
write_machine_record() {
    local name print commit key
    name=$(machine_name_of "$1"); commit=$(disk_commit "$1")
    print=$(head -1 "$1/system/effector/machine" 2> /dev/null | tr -cd '[:print:]' | head -c 96) || true
    key=${DISK_KEY_LABEL:-$(record_field "$MACHINE_DIR/machine.txt" device-key)}
    (umask 077; printf 'id %s\nhostname %s\nprint %s\nsaved %s\ncommit %s\ndevice-key %s\n' "$MACHINE" "${name:--}" "${print:--}" "$(date -u +%Y-%m-%dT%H:%MZ)" "${commit:--}" "${key:--}" > "$MACHINE_DIR/machine.txt.next") \
        && mv "$MACHINE_DIR/machine.txt.next" "$MACHINE_DIR/machine.txt"
}
# The device key labels known for the kept machine at $1: its machine.txt's and its kept device.key's.
machine_labels() {
    local l
    l=$(record_field "$1/machine.txt" device-key)
    [[ $l == "MIND "* ]] && echo "$l"
    key_label "$1/keystore/device.key" || true
}
# One device key on one machine (801-LNK-0046): when MACHINE's kept key, by its public label, is another kept machine's
# too, the write is refused before the writer opens, both named. A sealed key whose label is unknown is said.
KEY_FILES=(device.key device.sealed device.locked)
check_device_key() {
    local f d other label mine has=0 found=0
    for f in "${KEY_FILES[@]}"; do [[ -s $KEEP_DIR/$f ]] && has=1; done
    (( has )) || return 0
    mine=$(machine_labels "$MACHINE_DIR" | sort -u)
    if [[ -z $mine ]]; then
        echo "keep-wifi: the public label of machine $MACHINE's sealed device key is not known (no DEVICE KEY READY line in its disk's log): a key that another machine holds too cannot be seen" >&2
        return 0
    fi
    for d in "$MACHINES_KEEP"/*/; do
        d=${d%/}; other=${d##*/}
        [[ -d $d && $other != "$MACHINE" ]] || continue
        while IFS= read -r label; do
            [[ -n $label ]] && grep -qxF -- "$label" <<< "$mine" || continue
            echo ">>> Machines $MACHINE and $other hold the same device key ($label): one key on two machines is one identity on both (TLS, SSH, the Wi-Fi keys wrapped under it). Nothing is written. Write the machine that is to get a new key with --new-device-key: it makes a new one, and its remembered networks are joined again." >&2
            found=1
        done < <(machine_labels "$d" | sort -u)
    done
    (( ! found ))
}
# --new-device-key: the machine's kept device key and the networks wrapped under it moved aside, never removed, so that
# keystore makes a new key on the disk written.
set_key_aside() {
    local aside f g n=0
    aside="$KEEP_DIR/replaced-$(date -u +%Y%m%dT%H%M%SZ)"
    (umask 077; mkdir -p "$aside") || return 1
    for f in "${KEY_FILES[@]}" wifi.kept; do
        for g in "$KEEP_DIR/$f" "$KEEP_DIR/$f.prev"; do if [[ -e $g ]]; then mv "$g" "$aside/" && n=$((n + 1)); fi; done
    done
    if [[ -f $MACHINE_DIR/machine.txt ]]; then sed -i 's/^device-key .*/device-key -/' "$MACHINE_DIR/machine.txt"; fi
    DISK_KEY_LABEL=""
    echo "keep-wifi: --new-device-key: machine $MACHINE's device key and remembered networks ($n file(s)) set aside in $aside; keystore makes a new key on the disk written"
}
# One field of a machine.txt ($1), or - when it has none.
record_field() {
    local v
    v=$(sed -n "s/^$2 //p" "$1" 2> /dev/null | head -1) || true
    echo "${v:--}"
}
# The machines whose settings are kept: the ID, the host name, the date saved, the parts kept and the device key's
# public label; never a file's content.
list_machines() {
    local d id p parts n=0 row='%-24s %-20s %-18s %-36s %s\n'
    for d in "$MACHINES_KEEP"/*/; do
        [[ -d $d ]] || continue
        d=${d%/}; id=${d##*/}; parts=""
        for p in settings effector keystore netpolicy; do [[ -d $d/$p ]] && parts+=",$p"; done
        (( n++ )) || printf "$row" ID HOSTNAME SAVED KEPT "DEVICE KEY"
        printf "$row" "$id" "$(record_field "$d/machine.txt" hostname)" "$(record_field "$d/machine.txt" saved)" "${parts#,}" "$(record_field "$d/machine.txt" device-key)"
    done
    (( n )) || echo "No machine's settings are kept in $MACHINES_KEEP yet: a write with --keep-settings (or --keep-wifi, --keep-effector) from a machine's own disk keeps them."
    return 0
}
# Which machine this write keeps and puts on (801-LNK-0046), from $1, the machine the old disk names (empty: none, or no
# disk read), $2 (1: its volume was read) and --machine: sets MACHINE, and READ_OLD to 1 when the old disk's files go
# into that machine's directory. Fails, so that nothing is written, when no machine is known: never the one saved last.
READ_OLD=0
choose_machine() {
    local named=$1 seen=$2
    READ_OLD=0
    if [[ -n $named ]]; then
        if [[ -z $MACHINE || $MACHINE == "$named" ]]; then MACHINE=$named; READ_OLD=1; return 0; fi
        echo ">>> Disk $OLD_DISK holds machine $named's settings, not $MACHINE's (--machine): nothing is saved from it, and $MACHINES_KEEP/$named stays as it was." >&2
    elif [[ $seen == 1 ]]; then
        echo ">>> Disk $OLD_DISK names no machine (no Effector agent ID in system/effector/agent_id or data/effector/agent_id, no name in data/machine/name): nothing is saved from it." >&2
    fi
    if [[ -z $MACHINE ]]; then
        echo ">>> No machine to keep the settings of: no old disk naming one was read, and no --machine <id> was given. scripts/build_and_write.sh --list-machines lists the kept ones. Nothing is written." >&2
        return 1
    fi
    if [[ ! -d $MACHINES_KEEP/$MACHINE ]]; then
        echo ">>> Nothing is kept for machine $MACHINE (--machine): scripts/build_and_write.sh --list-machines lists the kept ones. Nothing is written." >&2
        return 1
    fi
    echo "keep: machine $MACHINE's settings (host name $(record_field "$MACHINES_KEEP/$MACHINE/machine.txt" hostname), saved $(record_field "$MACHINES_KEEP/$MACHINE/machine.txt" saved)) go on the disk written"
    return 0
}
# The one store of before 801-LNK-0046 is never used for a machine: said, and left as it is.
old_stores_note() {
    local d found=()
    for d in settings effector keystore netpolicy; do [[ -e $HOME/.config/mind/$d ]] && found+=("~/.config/mind/$d"); done
    (( ${#found[@]} )) && echo "keep: ${found[*]}, the one store of before 801-LNK-0046, left untouched and unused: each machine's settings are kept in $MACHINES_KEEP/<id>/"
    return 0
}

# The remembered Wi-Fi networks (keystore's files, 550-NET-0024), carried across a write through WSL's mount of the
# volume, so their bytes are on no command line; the mount is undone at once (the writer needs the volume unlocked).
KEPT_FILES=(wifi.kept device.key device.sealed device.locked wifi.country)
# The old disk's volume MIND CORE before the write, in OLD_LETTER. The disk is often still in its machine until the
# writer asks for it (800-LNK-0023), so the script asks for it now and waits up to OLD_DISK_WAIT seconds; Enter goes on
# without it. Called once, before the saves.
OLD_DISK_WAIT="${OLD_DISK_WAIT:-600}"
OLD_LETTER=""
find_old_volume() {
    # The disk's state: its volume MIND CORE's letter; `mounting` while that volume has none yet; `fresh` when the disk
    # holds no such volume (a new stick, another system); `absent` when there is no such disk.
    local ps="\$ProgressPreference = 'SilentlyContinue'; if (-not (Get-Disk -Number $OLD_DISK -ErrorAction SilentlyContinue)) { 'absent' } else { \$m = Get-Partition -DiskNumber $OLD_DISK -ErrorAction SilentlyContinue | Get-Volume -ErrorAction SilentlyContinue | Where-Object { \$_.FileSystemLabel -eq 'MIND CORE' } | Select-Object -First 1; if (\$m -and \$m.DriveLetter) { \$m.DriveLetter } elseif (\$m) { 'mounting' } else { 'fresh' } }"
    local state fresh=0
    state=$(winps "$ps" | tr -d '\r\n')
    if [[ ! $state =~ ^[A-Z]$ && $state != fresh ]]; then
        if [[ $OLD_GIVEN == 1 ]]; then echo ">>> Insert the OLD disk, the one the machine ran from, as disk $OLD_DISK now (not the new one yet): what it keeps (--keep-wifi, --keep-effector, --keep-settings) is saved from it first. Waiting up to $((OLD_DISK_WAIT / 60)) min; Enter goes on without it."
        else echo ">>> Insert the disk to be written (disk $DISK) now: what it keeps (--keep-wifi, --keep-effector, --keep-settings) is saved from it before the write. Waiting up to $((OLD_DISK_WAIT / 60)) min; Enter goes on without it."; fi
    fi
    local end=$((SECONDS + OLD_DISK_WAIT)) tty=0
    # Opened only when it opens: a failed exec redirection would end the script.
    if (: < /dev/tty) 2> /dev/null; then exec 3< /dev/tty; tty=1; fi
    while [[ ! $state =~ ^[A-Z]$ ]] && (( SECONDS < end )); do
        # A disk with no volume MIND CORE, seen twice 3 s apart (Windows names a volume a moment after the disk), has
        # nothing to save: the write goes on at once.
        if [[ $state == fresh ]]; then
            if (( ++fresh >= 2 )); then echo ">>> Disk $OLD_DISK holds no volume MIND CORE (a new disk): nothing is saved from it."; break; fi
        else fresh=0; fi
        if (( tty )); then
            if read -r -t 3 _ <&3; then echo ">>> Going on without the old disk: nothing is saved from it."; break; fi
        else sleep 3; fi
        state=$(winps "$ps" | tr -d '\r\n')
        [[ $state =~ ^[A-Z]$ ]] && echo ">>> Disk $OLD_DISK is here, its volume MIND CORE at $state:"
    done
    (( tty )) && exec 3<&-
    OLD_LETTER=""
    [[ $state =~ ^[A-Z]$ ]] && OLD_LETTER=$state
    return 0
}
# Disk __DISK__'s state (--old-disk): `old` while it is the device named __OLD__ (Windows' unique ID, serial number
# and size, in A-Z a-z 0-9 _ |), `here <its identity>` when it is another device, or `absent`. The old one may stay in.
read -r -d '' PS_DISK <<'PS' || true
$ProgressPreference = 'SilentlyContinue'
function DiskId($d) { "$($d.UniqueId)|$($d.SerialNumber)|$($d.Size)" -replace '[^A-Za-z0-9|]', '_' }
$old = '__OLD__'
$d = Get-Disk -Number __DISK__ -ErrorAction SilentlyContinue
if (-not $d) { 'absent' } elseif ($old -and (DiskId $d) -eq $old) { 'old' } else { "here $(DiskId $d)" }
PS
disk_state() { local s="${PS_DISK//__OLD__/$2}"; winps "${s//__DISK__/$1}" | tr -d '\r\n'; }
OLD_IDENT=""
# With --old-disk, the old disk read: the new one is asked for as disk $DISK, and waited on until disk $DISK is there
# and it is another device; the steps after the write find the new volume by this build's MANIFEST, so the old one may stay in.
wait_new_disk() {
    local state end=$((SECONDS + OLD_DISK_WAIT)) tty=0 said="" went=0 shown=$SECONDS
    if [[ -n $OLD_IDENT ]]; then state=$(disk_state "$DISK" "$OLD_IDENT"); else state=unknown; fi
    [[ $state == here* ]] && { echo ">>> Disk $DISK is another device than the old disk: it is written."; return 0; }
    echo ">>> The old disk (disk $OLD_DISK) is read. Insert the NEW disk to be written as disk $DISK; the old one may stay in. Waiting up to $((OLD_DISK_WAIT / 60)) min, the state shown every 15 s; Enter goes on as it is."
    if (: < /dev/tty) 2> /dev/null; then exec 3< /dev/tty; tty=1; fi
    while [[ $state != here* ]] && (( SECONDS < end )); do
        if [[ $state == absent && $said != absent ]]; then
            echo ">>> The old disk is out, and there is no disk $DISK yet. If the new one takes another number (scripts/build_and_write.sh --list), stop with Ctrl+C and run again with --machine $MACHINE and that number: the settings are kept already."
        fi
        said=$state
        (( SECONDS - shown >= 15 )) && { shown=$SECONDS; echo ">>> Waiting for disk $DISK: $([[ $state == old ]] && echo "it is the old disk" || { [[ $state == absent ]] && echo "not there" || echo "$state"; })"; }
        if (( tty )); then
            if read -r -t 3 _ <&3; then went=1; echo ">>> Going on as it is: the writer shows disk $DISK and asks before it erases it."; break; fi
        else sleep 3; fi
        # Without the old disk's identity, only Enter says that the new one is in.
        [[ -n $OLD_IDENT ]] && state=$(disk_state "$DISK" "$OLD_IDENT")
    done
    (( tty )) && exec 3<&-
    [[ $state == here* ]] && echo ">>> Disk $DISK is the new disk, another device than the old one."
    [[ $state == here* || $went == 1 ]] && return 0
    echo ">>> No new disk as disk $DISK within $((OLD_DISK_WAIT / 60)) min: nothing is written. Machine $MACHINE's settings are kept: run again with --machine $MACHINE." >&2
    return 1
}
# The old disk's volume read once (801-LNK-0046): the machine it names, then what each --keep option keeps, into that
# machine's directory. Fails when no machine is known, or its directory cannot be written, before anything is written.
read_old_disk() {
    local dir="" named="" seen=0
    if [[ $OLD_LETTER =~ ^[A-Z]$ ]]; then
        dir=$(mktemp -d)
        if sudo mount -t drvfs "$OLD_LETTER:" "$dir" -o "uid=$(id -u),gid=$(id -g)"; then seen=1; named=$(machine_id_of "$dir") || named=""
        else echo ">>> Disk $OLD_DISK's volume $OLD_LETTER: could not be mounted: nothing is saved from it." >&2; rmdir "$dir"; dir=""; fi
    fi
    local known=0
    DISK_KEY_LABEL=""
    if choose_machine "$named" "$seen"; then
        known=1; use_machine "$MACHINE"
        # The key the disk holds, by its public label: from device.key, else from the log (a sealed key, or no openssl).
        if [[ $READ_OLD == 1 && $KEEP_WIFI == 1 ]]; then
            local f any=0
            for f in "${KEY_FILES[@]}"; do [[ -s $dir/system/keystore/$f ]] && any=1; done
            DISK_KEY_LABEL=$(key_label "$dir/system/keystore/device.key") || { (( any )) && DISK_KEY_LABEL=$(old_log_label); } || DISK_KEY_LABEL=""
        fi
        # The disk written may be the old one: what could not be kept is not erased.
        if [[ $READ_OLD == 1 ]] && ! save_machine_from "$dir"; then echo ">>> Machine $MACHINE's settings NOT saved from disk $OLD_DISK into $MACHINE_DIR: nothing is written." >&2; known=0; fi
    fi
    if [[ -n $dir ]]; then sudo umount "$dir"; rmdir "$dir"; fi
    if [[ $known == 1 && $KEEP_WIFI == 1 ]]; then
        if [[ $NEW_KEY == 1 ]] && ! set_key_aside; then echo ">>> Machine $MACHINE's device key NOT set aside: nothing is written." >&2; known=0; fi
        if [[ $known == 1 ]] && ! check_device_key; then known=0; fi
    fi
    [[ $known == 1 ]]
}
# The volume at $1 saved into MACHINE's directory, part by part as the --keep options ask, with its machine.txt.
save_machine_from() {
    local vol=$1 n name
    (umask 077; mkdir -p "$MACHINE_DIR") && chmod 700 "$MACHINES_KEEP" "$MACHINE_DIR" || return 1
    name=$(machine_name_of "$vol")
    echo "keep: disk $OLD_DISK is machine $MACHINE (host name ${name:--}): its settings are kept in $MACHINE_DIR, and go on the disk written"
    if [[ $KEEP_WIFI == 1 ]]; then keep_keystore_from "$vol"; fi
    if [[ $KEEP_EFFECTOR == 1 ]]; then
        if n=$(keep_effector_from "$vol"); then echo "keep-effector: $n file(s) of $OLD_LETTER:\\data\\effector and \\system\\effector saved in $EFFECTOR_KEEP"
        else echo "keep-effector: $OLD_LETTER: holds no Effector agent; what was saved before for $MACHINE will be put back"; fi
    fi
    if [[ $KEEP_SETTINGS == 1 ]]; then
        if n=$(keep_settings_from "$vol"); then
            echo "keep-settings: $n file(s) of $OLD_LETTER:\\data and \\system saved in $SETTINGS_KEEP ($(du -sh "$SETTINGS_KEEP" | cut -f1)): $(ls "$SETTINGS_KEEP/data" | tr '\n' ' ')"
        else echo "keep-settings: $OLD_LETTER: holds no settings; what was saved before for $MACHINE will be put back"; fi
    fi
    write_machine_record "$vol"
}
# Keystore's files and the network policy as changed, from the volume at $1 into the machine's keystore/ and netpolicy/.
keep_keystore_from() {
    local dir=$1 n=0 f any=0
    for f in "${KEPT_FILES[@]}"; do [[ -f $dir/system/keystore/$f ]] && any=1; done
    if (( any )); then
        (umask 077; mkdir -p "$KEEP_DIR") && chmod 700 "$KEEP_DIR"
        for f in "${KEPT_FILES[@]}"; do
            keep_file "$dir/system/keystore/$f" "$KEEP_DIR/$f" && n=$((n + 1))
        done
        echo "keep-wifi: $n file(s) of $OLD_LETTER:\\system\\keystore saved in $KEEP_DIR (device key ${DISK_KEY_LABEL:-of no known label})"
    else
        echo "keep-wifi: $OLD_LETTER: holds no device key or remembered network; $(ls "$KEEP_DIR" 2>/dev/null | wc -l) file(s) saved before for $MACHINE will be put back"
    fi
    if [[ -f $dir/system/netpolicy/netpolicy.txt ]]; then
        (umask 077; mkdir -p "$POLICY_KEEP") && chmod 700 "$POLICY_KEEP"
        (umask 077; tr -d '\r' < "$dir/system/netpolicy/netpolicy.txt" > "$POLICY_KEEP/changed.txt")
        if [[ -f $dir/netpolicy.txt ]]; then (umask 077; tr -d '\r' < "$dir/netpolicy.txt" > "$POLICY_KEEP/shipped.txt"); else (umask 077; : > "$POLICY_KEEP/shipped.txt"); fi
        echo "keep-wifi: the network policy as changed on $OLD_LETTER: saved in $POLICY_KEEP ($(policy_changes added) added, $(policy_changes removed) removed)"
    elif [[ -f $POLICY_KEEP/changed.txt ]]; then
        echo "keep-wifi: $OLD_LETTER: the network policy is not changed there; the changes saved before for $MACHINE will be put back"
    fi
    return 0
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
    local letter dir status=0 f any=0
    for f in "${KEPT_FILES[@]}"; do [[ -f $KEEP_DIR/$f ]] && any=1; done
    (( any )) || { echo "keep-wifi: nothing saved in $KEEP_DIR: join by hand once, then write with --keep-wifi" >&2; return 1; }
    letter=$(winps "${PS_LETTER//__MANIFEST__/$(sha256sum "$MANIFEST_FILE" | cut -d' ' -f1)}" | tr -d '\r\n')
    [[ $letter =~ ^[A-Z]$ ]] || { echo "keep-wifi: no volume MIND CORE holding this build (its MANIFEST) within 60 s" >&2; return 1; }
    dir=$(mktemp -d)
    sudo mount -t drvfs "$letter:" "$dir" -o "uid=$(id -u),gid=$(id -g)" || { rmdir "$dir"; return 1; }
    put_keystore_to "$dir" "$letter" || status=1
    sudo umount "$dir"; rmdir "$dir"
    winps "\$ProgressPreference = 'SilentlyContinue'; Write-VolumeCache -DriveLetter $letter" > /dev/null || true
    return $status
}
# Puts the machine's keystore files, its network policy merged with the new image's and the join at start on the
# volume at $1 (its letter $2, for the messages), each read back.
put_keystore_to() {
    local dir=$1 letter=$2 status=0 f
    mkdir -p "$dir/system/keystore"
    for f in "${KEPT_FILES[@]}"; do
        [[ -f $KEEP_DIR/$f ]] || continue
        cp "$KEEP_DIR/$f" "$dir/system/keystore/$f" && sync && cmp -s "$KEEP_DIR/$f" "$dir/system/keystore/$f" || { echo "keep-wifi: $f NOT put back" >&2; status=1; }
    done
    [[ $status == 0 ]] && echo "keep-wifi: the device key and the remembered Wi-Fi networks put back on $letter:\\system\\keystore, read back"
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
    elif [[ ! -f $KEEP_DIR/wifi.kept ]]; then echo "keep-wifi: no remembered network kept for machine $MACHINE: join by hand, or write with --wifijoin" >&2
    elif [[ -z $name ]]; then echo "keep-wifi: no network's name (KEEP_WIFI_NAME, or the first line of $WIFIJOIN_CONF): no join at start" >&2
    elif ! grep -q 'REMEMBERED BY KEYSTORE' tests/hardware/wifijoin.msh 2>/dev/null; then echo "keep-wifi: this tree's wifijoin.msh cannot join a remembered network yet: join by hand" >&2
    else
        mkdir -p "$dir/data" && printf '%s\n' "$name" > "$dir/data/wifijoin.txt" && cp tests/hardware/wifijoin.msh "$dir/autorun.msh" && sync \
            && cmp -s tests/hardware/wifijoin.msh "$dir/autorun.msh" \
            && echo "keep-wifi: autorun.msh joins \"$name\" at start (data/wifijoin.txt holds the name alone)" \
            || { echo "keep-wifi: the join at start NOT written" >&2; status=1; }
    fi
    return $status
}

# The Effector agent's files (800-LNK-0023), carried across a write as keystore's are: the owner's configuration and
# policy in data/effector/, the agent's own state in system/effector/. Only the owner's shell writes them on the machine.
# Copies the volume at $1's agent files into $EFFECTOR_KEEP, whole or not at all; echoes how many files.
keep_effector_from() {
    local vol=$1 next="$EFFECTOR_KEEP.next" n
    [[ -d $vol/data/effector || -d $vol/system/effector ]] || return 1
    local part
    rm -rf "$next"; (umask 077; mkdir -p "$next/data" "$next/system")
    for part in data system; do
        if has_content "$vol/$part/effector"; then
            (umask 077; cp -r "$vol/$part/effector/." "$next/$part/") && refill_empty "$next/$part" "$EFFECTOR_KEEP/$part"
        elif has_content "$EFFECTOR_KEEP/$part"; then
            (umask 077; cp -r "$EFFECTOR_KEEP/$part/." "$next/$part/")
            echo "keep-effector: the disk's $part/effector is missing or empty: what was saved before is kept" >&2
        fi
    done
    find "$next" -type d -exec chmod 700 {} + && find "$next" -type f -exec chmod 600 {} +
    n=$(find "$next" -type f | wc -l)
    rotate_kept "$next" "$EFFECTOR_KEEP" && echo "$n"
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
SETTINGS_SKIP_DATA=(effector firmware wallpapers link wifitest.txt wifi-ofdm.txt wifijoin.txt)
SETTINGS_SKIP_SYSTEM=(keystore netpolicy effector)
# Copies the volume at $1's settings into $SETTINGS_KEEP, whole or not at all; echoes how many files.
keep_settings_from() {
    local vol=$1 next="$SETTINGS_KEEP.next" sub x args
    [[ -d $vol/data || -d $vol/system ]] || return 1
    rm -rf "$next"; (umask 077; mkdir -p "$next/data" "$next/system")
    for sub in data system; do
        if [[ -d $vol/$sub ]]; then
            args=()
            if [[ $sub == data ]]; then for x in "${SETTINGS_SKIP_DATA[@]}"; do args+=("--exclude=./$x"); done
            else for x in "${SETTINGS_SKIP_SYSTEM[@]}"; do args+=("--exclude=./$x"); done; fi
            (cd "$vol/$sub" && tar cf - "${args[@]}" .) | (umask 077; cd "$next/$sub" && tar xf - --no-same-owner) || { rm -rf "$next"; return 1; }
        fi
        if ! has_content "$next/$sub" && has_content "$SETTINGS_KEEP/$sub"; then
            (umask 077; cp -r "$SETTINGS_KEEP/$sub/." "$next/$sub/")
            echo "keep-settings: the disk's $sub/ holds none of the settings: what was saved before is kept" >&2
        fi
    done
    find "$next" -type d -exec chmod 700 {} + && find "$next" -type f -exec chmod 600 {} +
    rotate_kept "$next" "$SETTINGS_KEEP" && find "$SETTINGS_KEEP" -type f | wc -l
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
# The kept machines (801-LNK-0046): nothing is fetched, built or written.
if [[ "${1:-}" == "--list-machines" ]]; then list_machines; exit 0; fi
[[ ($LINK_RUN == 1 || $SLOTS == 1 || -n $LINK_ADDRESS) && -z $LINK_BAUD ]] && LINK_BAUD=460800
[[ $SLOTS == 1 && -n $IMAGE_FILE ]] && { echo "--slots builds the image; it does not go with --image" >&2; exit 1; }
DISK="${1:?usage: build_and_write.sh [--commit <id>] [--slots] [--wifitest] [--wifi-ofdm[=<cores off>]] [--wifijoin] [--link[=<baud>]] [--link-run] [--link-dev] [--link-address=<n>] [--keep-wifi] [--keep-effector] [--keep-settings] [--machine <id>] [--old-disk <n>] [--new-device-key] [--mode=<modes>] <disk number> | --list | --list-machines}"
[[ "$DISK" =~ ^[0-9]+$ ]] || { echo "Disk number must be digits: $DISK" >&2; exit 1; }
# The old disk is the one written unless --old-disk names another device.
OLD_DISK=${OLD_DISK:-$DISK}

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
[[ -n "$COMMIT" ]] && echo ">>> Building commit $(git log -1 --format='%h %ad %s' --date=short)"
echo ">>> build_and_write.sh $(git log -1 --format=%h -- scripts/build_and_write.sh) runs$([[ -n $UPDATED ]] && { [[ -n $RESTORE_BRANCH ]] && echo ", the commit's own" || echo ', the version the pull brought'; })"
# Build first, then pack: make_usb_image.py lists the programs when it starts, so packing inside its own build misses new ones.
bash ./02_build.sh
# With --slots: the build in slot A and an update.txt that takes releases from data/link/rel/ (scripts/link_update.py),
# looked for once a minute: each check reads the disk and logs two lines (800-LNK-0040).
SLOT_ARGS=()
if [[ $SLOTS == 1 ]]; then
    UPDATE_TXT="$(mktemp)"
    printf 'source data/link/rel\nchannel stable\nautomatic apply\nevery 60\n' > "$UPDATE_TXT"
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

# Each machine's settings by its identity (801-LNK-0046): the old disk read into its machine's directory, then that
# machine's, or --machine's, put on the disk written; with no machine known, nothing is written.
if [[ $KEEP_WIFI == 1 || $KEEP_EFFECTOR == 1 ]]; then
    old_stores_note
    find_old_volume
    if [[ $OLD_GIVEN == 1 && -n $OLD_LETTER ]]; then
        OLD_IDENT=$(disk_state "$OLD_DISK" "") || OLD_IDENT=""
        if [[ $OLD_IDENT == "here "* ]]; then OLD_IDENT=${OLD_IDENT#here }; else OLD_IDENT=""; fi
    fi
    read_old_disk || exit 1
    if [[ $OLD_GIVEN == 1 && -n $OLD_LETTER ]]; then wait_new_disk || exit 1; fi
fi
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
# An Intel Mac's startup disk (800-LNK-0029): mac/startup-disk.sh, run in macOS Recovery, makes the Mac start this disk.
if [[ -z $IMAGE_FILE && -x scripts/disk_copy.sh ]]; then
    scripts/disk_copy.sh scripts/mac mac mac || echo ">>> mac/startup-disk.sh NOT copied: run scripts/disk_copy.sh scripts/mac mac mac." >&2
fi
for flag in "${FLAGS[@]}"; do put_flag "$flag" || echo ">>> data/${flag%%:*} NOT put on the disk." >&2; done
if [[ $WIFIJOIN == 1 ]]; then put_wifijoin || echo ">>> The Wi-Fi network NOT put on the disk." >&2; fi
if [[ -n $LINK_BAUD ]]; then put_link "$LINK_BAUD" || echo ">>> link.conf NOT put on the disk." >&2; fi
if [[ $KEEP_WIFI == 1 ]]; then put_keystore || echo ">>> The remembered Wi-Fi networks NOT put on the disk." >&2; fi
if [[ $KEEP_EFFECTOR == 1 ]]; then put_effector || echo ">>> The Effector agent's files NOT put on the disk." >&2; fi
