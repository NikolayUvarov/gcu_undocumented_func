#requires -Version 5.1
<#
.SYNOPSIS
Write the MIND CORE RAW image to a selected USB disk and verify SHA256.
.EXAMPLE
.\05_write_usb_windows.ps1 -List
.EXAMPLE
.\05_write_usb_windows.ps1 -DiskNumber 2 -Check
.EXAMPLE
.\05_write_usb_windows.ps1 -DiskNumber 2
#>
[CmdletBinding()]
param(
    [switch]$List,
    [int]$DiskNumber = -1,
    [string]$Image = (Join-Path $PSScriptRoot 'dist\mind-core-usb.img'),
    [switch]$Check
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Assert-UsbTarget($Disk, [long]$ImageSize, [int[]]$ProtectedDisks) {
    if ($Disk.BusType -ne 'USB' -or $Disk.IsBoot -or $Disk.IsSystem) {
        throw 'Select a non-system USB disk. Internal and boot/system disks are refused.'
    }
    if ($Disk.IsReadOnly -or $Disk.IsOffline -or $Disk.LogicalSectorSize -ne 512 -or $Disk.Size -lt $ImageSize) {
        throw 'Disk is read-only/offline, uses non-512-byte sectors, or is smaller than the image.'
    }
    if ($ProtectedDisks -contains $Disk.Number) {
        throw 'The selected disk contains the image or writer script. Move them to another disk.'
    }
}

function Get-Identity($Disk) {
    return '{0}|{1}|{2}|{3}|{4}|{5}' -f $Disk.Number, $Disk.UniqueId, $Disk.SerialNumber, $Disk.Size, $Disk.Path, $Disk.FriendlyName
}

function Get-ProtectedDisks([string]$ImagePath) {
    $result = @()
    foreach ($path in @($ImagePath, $PSScriptRoot)) {
        $volume = @(Get-Volume -FilePath $path)
        if ($volume.Count -ne 1) { throw "Cannot identify the volume containing $path" }
        $partitions = @($volume | Get-Partition)
        if ($partitions.Count -eq 0) { throw "Cannot identify the disk containing $path" }
        $result += @($partitions | Select-Object -ExpandProperty DiskNumber)
    }
    return @($result | Sort-Object -Unique)
}

function Invoke-UsbWriter {
    if ($env:OS -ne 'Windows_NT') { throw 'Use the Linux .sh writer on Linux.' }
    if ($List -or $DiskNumber -lt 0) {
        Get-Disk | Where-Object BusType -eq 'USB' |
            Select-Object Number, FriendlyName, SerialNumber, Size, IsBoot, IsSystem, IsReadOnly |
            Format-Table -AutoSize | Out-Host
        if (-not $List) { Write-Host 'Choose explicitly: .\05_write_usb_windows.ps1 -DiskNumber N [-Check]' }
        return
    }
    $item = Get-Item -LiteralPath $Image
    if ($item.PSIsContainer -or $item -isnot [System.IO.FileInfo]) { throw 'Image must be a regular file.' }
    $resolvedImage = $item.FullName
    $disk = Get-Disk -Number $DiskNumber
    Assert-UsbTarget $disk $item.Length (Get-ProtectedDisks $resolvedImage)
    $identity = Get-Identity $disk
    if (-not ('UsbImageWriter' -as [type])) {
        Add-Type -Path (Join-Path $PSScriptRoot 'scripts\UsbImageWriter.cs')
    }
    # Keep the source open without write/delete sharing throughout confirmation and I/O.
    $source = [System.IO.File]::Open($resolvedImage, [System.IO.FileMode]::Open,
        [System.IO.FileAccess]::Read, [System.IO.FileShare]::Read)
    try {
        $hash = [UsbImageWriter]::ImageHash($source)
        Write-Host "Image: $resolvedImage ($($source.Length) bytes)"
        Write-Host "SHA256: $hash"
        $disk | Select-Object Number, FriendlyName, SerialNumber, Size, BusType | Format-List | Out-Host
        if ($Check) { Write-Host 'Check passed. Nothing was unmounted or written.'; return }
        $principal = [Security.Principal.WindowsPrincipal]::new([Security.Principal.WindowsIdentity]::GetCurrent())
        if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
            throw 'Open PowerShell as Administrator to write. -List and -Check do not write.'
        }
        Write-Host 'ALL DATA ON THIS USB DISK WILL BE LOST.' -ForegroundColor Yellow
        if ((Read-Host "Type ERASE $DiskNumber to continue") -cne "ERASE $DiskNumber") {
            throw 'Cancelled. Nothing was written.'
        }
        $disk = Get-Disk -Number $DiskNumber
        Assert-UsbTarget $disk $source.Length (Get-ProtectedDisks $resolvedImage)
        if ((Get-Identity $disk) -cne $identity) { throw 'Disk changed after selection. Run again.' }
        # An unpartitioned disk is valid; querying by number can report
        # ObjectNotFound instead of an empty array on some Storage versions.
        $partitions = @(Get-Partition | Where-Object DiskNumber -eq $DiskNumber)
        $paths = @()
        foreach ($partition in $partitions) {
            if ($partition.IsBoot -or $partition.IsSystem) { throw 'A system partition was found. Refusing.' }
            $paths += @($partition.AccessPaths | Where-Object { $_ -like '\\?\Volume{*}\' })
            $volumes = @()
            try { $volumes = @(Get-Volume -Partition $partition) }
            catch {
                # An unrecognized/unformatted partition may have no Windows volume.
                if ($_.CategoryInfo.Category -ne 'ObjectNotFound') { throw }
            }
            foreach ($volume in $volumes) {
                if ($volume.Path) { $paths += $volume.Path }
            }
        }
        $paths = @($paths | Sort-Object -Unique)
        $progress = [Action[int]] {
            param($Percent)
            Write-Progress -Activity 'Writing USB image' -PercentComplete $Percent -Status "$Percent%"
        }
        [UsbImageWriter]::WriteDisk($source, $DiskNumber, $disk.Size, $hash, [string[]]$paths, $progress)
        Write-Progress -Activity 'Writing USB image' -Completed
        Write-Host 'Done. Read-back SHA256 matches. Safely remove/reconnect the USB drive.'
        Write-Host 'Boot in UEFI x64 mode with Secure Boot disabled.'
    }
    finally { $source.Dispose() }
}

# Dot-sourcing exposes validation functions for non-destructive tests only.
if ($MyInvocation.InvocationName -ne '.') {
    try { Invoke-UsbWriter }
    catch { Write-Error -Message $_.Exception.Message -ErrorAction Continue; exit 1 }
}
