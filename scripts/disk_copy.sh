#!/usr/bin/env bash
# Copies files onto a written disk's boot volume after the image is written, and reads each one back by SHA-256: what
# the image must not hold (proprietary files, AGENTS.md section 3; desktop pictures, wallpapers/README.md).
# Usage: scripts/disk_copy.sh SOURCE DEST TAG          onto the volume labelled MIND CORE that holds the image in usb_root/
#                                                       (Windows, from WSL), under DEST (for example data/firmware)
#        scripts/disk_copy.sh SOURCE DEST TAG VOLUME   under VOLUME/DEST (a mounted boot volume on Linux)
# SOURCE's files are copied with their subdirectories; with ONLY=PATTERN (a find -name pattern), only those matching.
# Each file copied is listed with its size once all are read back.
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
[[ $# -ge 3 ]] || { sed -n '2,7p' "$0" | sed 's/^# \{0,1\}//'; exit 2; }
DEST="${2#/}"; TAG="$3"
fail() { echo "$TAG: $*" >&2; exit 1; }
[[ -d $1 ]] || fail "no directory $1"
SOURCE="$(cd -- "$1" && pwd)"
mapfile -t names < <(cd "$SOURCE" && find . -type f -name "${ONLY:-*}" | sed 's|^\./||' | sort)
(( ${#names[@]} )) || fail "nothing to copy in $SOURCE"

if [[ $# -ge 4 ]]; then
    for name in "${names[@]}"; do mkdir -p "$4/$DEST/$(dirname -- "$name")" && cp "$SOURCE/$name" "$4/$DEST/$name"; done
    (cd "$SOURCE" && sha256sum "${names[@]}") | (cd "$4/$DEST" && sha256sum --quiet -c -) || fail "the copies under $4/$DEST differ"
    for name in "${names[@]}"; do echo "$TAG:   $4/$DEST/$name, $(stat -c%s "$4/$DEST/$name") bytes"; done
    echo "$TAG: copied under $4/$DEST, ${#names[@]} files read back with their SHA-256"
    exit 0
fi

# Windows may show the disk's earlier volume, with the same label, for a while after the writer ends, and files copied
# onto it are lost (2026-10-10): the volume is the one whose MANIFEST is the image's.
read -r -d '' PS_COPY <<'PS' || true
$ErrorActionPreference = 'Stop'; $ProgressPreference = 'SilentlyContinue'; trap { Write-Output "__TAG__: $_"; exit 1 }
$source = '__SOURCE__'; $manifest = '__MANIFEST__'; $files = @(__FILES__)
for ($i = 0; $i -lt 30; $i++) {
    $v = Get-Volume | Where-Object { $_.FileSystemLabel -eq 'MIND CORE' -and $_.DriveLetter } | Select-Object -First 1
    if ($v -and -not $manifest) { break }
    if ($v -and (Test-Path "$($v.DriveLetter):\MANIFEST") -and (Get-FileHash "$($v.DriveLetter):\MANIFEST").Hash -eq $manifest) { break }
    $v = $null; Start-Sleep 2
}
if (-not $v) { Write-Output '__TAG__: no volume MIND CORE holding the image just built (its MANIFEST) within 60 s'; exit 1 }
$to = "$($v.DriveLetter):\__DEST__"
for ($try = 1; $try -le 3; $try++) {
    foreach ($f in $files) {
        $name = ($f -split ' ', 2)[1]; $target = Join-Path $to $name
        New-Item -ItemType Directory -Force (Split-Path $target) | Out-Null
        Copy-Item -Force (Join-Path $source $name) $target
    }
    Write-VolumeCache -DriveLetter $v.DriveLetter
    Start-Sleep 2
    $bad = @($files | Where-Object { $sum, $name = $_ -split ' ', 2; $path = Join-Path $to $name; -not (Test-Path $path) -or (Get-FileHash $path).Hash -ne $sum })
    if ($bad.Count -eq 0) {
        foreach ($f in $files) { $name = ($f -split ' ', 2)[1]; Write-Output ("__TAG__:   {0}\{1}, {2} bytes" -f $to, $name, (Get-Item (Join-Path $to $name)).Length) }
        Write-Output "__TAG__: copied to $to, $($files.Count) files read back with their SHA-256"; exit 0
    }
    Write-Output "__TAG__: try ${try}: $($bad.Count) files missing or different on $to"
}
exit 1
PS

command -v powershell.exe >/dev/null || fail "no PowerShell: give the mounted boot volume as the fourth argument"
files="" manifest=""
for name in "${names[@]}"; do files+="${files:+,}'$(sha256sum "$SOURCE/$name" | cut -d' ' -f1) ${name//\//\\}'"; done
[[ -f $ROOT/usb_root/MANIFEST ]] && manifest="$(sha256sum "$ROOT/usb_root/MANIFEST" | cut -d' ' -f1)"
script="${PS_COPY//__SOURCE__/$(wslpath -w "$SOURCE")}"; script="${script//__MANIFEST__/$manifest}"; script="${script//__FILES__/$files}"
script="${script//__DEST__/${DEST//\//\\}}"; script="${script//__TAG__/$TAG}"
status=0
powershell.exe -NoProfile -EncodedCommand "$(printf '%s' "$script" | iconv -f UTF-8 -t UTF-16LE | base64 -w0)" | tr -d '\r' || status=$?
(( status == 0 )) || fail "the files are not on the disk: copy them again before unplugging it"
