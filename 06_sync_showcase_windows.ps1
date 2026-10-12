# Copies main and the tags of this repository on GitHub to the showcase, github.com/NikolayUvarov/mind-core-os, with your
# own git access (your SSH key, or Git Credential Manager for an https address): only what changed, never forced, so a
# commit made in the showcase stops the copy instead of being overwritten (000-KRN-0088). The same steps as
# scripts/mirror_showcase.sh. Run it from any clone of this repository, on any branch:
#
#   powershell -ExecutionPolicy Bypass -File .\06_sync_showcase_windows.ps1
#   powershell -ExecutionPolicy Bypass -File .\06_sync_showcase_windows.ps1 -Showcase https://github.com/NikolayUvarov/mind-core-os.git
param(
    [string]$Showcase = "git@github.com:NikolayUvarov/mind-core-os.git",
    [string]$Source = "origin"
)
$ErrorActionPreference = "Stop"
$Repo = Split-Path -Parent $MyInvocation.MyCommand.Path

function Invoke-Git {
    $out = & git -C $Repo @args
    if ($LASTEXITCODE -ne 0) { throw "git $($args -join ' ') failed with $LASTEXITCODE" }
    $out
}

# main into a ref of its own, so the clone's branches are left as they are.
Invoke-Git fetch -q $Source "+refs/heads/main:refs/mirror/main" --tags | Out-Null
$main = "$(Invoke-Git rev-parse refs/mirror/main)".Trim()
$shown = ("$(Invoke-Git ls-remote $Showcase refs/heads/main)" -split "\s+")[0]
$local = @(Invoke-Git tag)
$remote = @(Invoke-Git ls-remote --tags --refs $Showcase | ForEach-Object { ($_ -split "refs/tags/")[1] })
$newTags = @($local | Where-Object { $_ -and ($remote -notcontains $_) })
if ($main -eq $shown -and $newTags.Count -eq 0) {
    Write-Host "mirror: nothing changed since the last copy (main at $($main.Substring(0, 7)))"
    exit 0
}
Invoke-Git push -q $Showcase "refs/mirror/main:refs/heads/main" | Out-Null
Invoke-Git push -q $Showcase --tags | Out-Null
Write-Host "mirror: $Showcase has main at $($main.Substring(0, 7)) and $($local.Count) tags"
