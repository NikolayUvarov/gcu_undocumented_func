# Test only policy, compilation and normal file I/O. Never calls WriteDisk.
$ErrorActionPreference = 'Stop'
$TestRoot = Split-Path -Parent $PSScriptRoot
. (Join-Path $TestRoot '05_write_usb_windows.ps1')
Add-Type -Path (Join-Path $TestRoot 'scripts\UsbImageWriter.cs')

function Must-Reject([scriptblock]$Action) {
    $rejected = $false
    try { & $Action } catch { $rejected = $true }
    if (-not $rejected) { throw 'Unsafe request was accepted.' }
}
function New-TestDisk {
    return [pscustomobject]@{ Number=9; BusType='USB'; IsBoot=$false; IsSystem=$false;
        IsReadOnly=$false; IsOffline=$false; LogicalSectorSize=512; Size=1073741824;
        UniqueId='TEST-ONLY'; SerialNumber='TEST-ONLY'; Path='TEST-ONLY'; FriendlyName='FAKE USB' }
}

$valid = New-TestDisk
Assert-UsbTarget $valid 1024 @(0)
foreach ($change in @(@('BusType','SATA'), @('IsBoot',$true), @('IsSystem',$true),
    @('IsReadOnly',$true), @('IsOffline',$true), @('LogicalSectorSize',4096), @('Size',512))) {
    $wrong = New-TestDisk
    $wrong.($change[0]) = $change[1]
    Must-Reject { Assert-UsbTarget $wrong 1024 @(0) }
}
Must-Reject { Assert-UsbTarget $valid 1024 @(9) }
Write-Host 'PASS: Windows USB/system/source/size/sector/read-only/offline policy'

$imagePath = Join-Path $TestRoot 'dist\mind-core-usb.img'
$source = [IO.File]::OpenRead($imagePath)
try {
    $hash = [UsbImageWriter]::ImageHash($source)
    if ($hash -ne (Get-FileHash -LiteralPath $imagePath -Algorithm SHA256).Hash) { throw 'Image hash mismatch' }
} finally { $source.Dispose() }
$protected = @(Get-ProtectedDisks $imagePath)
if (-not $protected.Count) { throw 'Failed to discover source disk' }
Write-Host 'PASS: real image validation and read-only source volume/disk discovery'

# Exercise the full -Check route using fake disk metadata. Confirmation and
# native WriteDisk are unreachable; Read-Host is replaced with a failure guard.
function Get-Disk { param([int]$Number); return New-TestDisk }
function Get-ProtectedDisks { param([string]$ImagePath); return @(0) }
function Read-Host { throw 'Dry-run must not request confirmation.' }
$Image = $imagePath
$DiskNumber = 9
$Check = $true
Invoke-UsbWriter
Write-Host 'PASS: Windows -Check exits before confirmation/volume locks/device write'

$work = Join-Path $TestRoot ('dist\writer-test-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $work > $null
try {
    $bytes = New-Object byte[] (5 * 1024 * 1024)
    for ($i = 0; $i -lt $bytes.Length; $i += 512) { $bytes[$i] = [byte](($i / 512) % 251) }
    $sha = [Security.Cryptography.SHA256]::Create()
    try { $expected = [BitConverter]::ToString($sha.ComputeHash($bytes)).Replace('-','').ToLowerInvariant() }
    finally { $sha.Dispose() }
    [IO.File]::WriteAllBytes((Join-Path $work 'source.bin'), $bytes)
    foreach ($extra in @(0, 65536)) {
        $source = [IO.File]::OpenRead((Join-Path $work 'source.bin'))
        $target = [IO.File]::Open((Join-Path $work 'target.bin'), 'Create', 'ReadWrite', 'None')
        try {
            $capacity = $bytes.Length + $extra
            $target.SetLength($capacity)
            $target.Position = $capacity - 1
            $target.WriteByte(0xff)
            [UsbImageWriter]::CopyAndVerify($source, $target, $bytes.Length, $capacity, $expected,
                [Action]{ $target.Flush($true) }, [Action[int]]{ param($n) })
            $target.Position = $capacity - 1
            if ($extra -gt 0 -and $target.ReadByte() -ne 0) { throw 'Old GPT tail not cleared' }
            Must-Reject {
                [UsbImageWriter]::CopyAndVerify($source, $target, $bytes.Length, $capacity, $expected,
                    [Action]{ $target.Position=0; $target.WriteByte(0xff); $target.Flush($true) },
                    [Action[int]]{ param($n) })
            }
            Must-Reject {
                [UsbImageWriter]::CopyAndVerify($source, $target, $bytes.Length, 512, $expected,
                    [Action]{}, [Action[int]]{ param($n) })
            }
        } finally { $target.Dispose(); $source.Dispose() }
    }
    Write-Host 'PASS: Windows multi-buffer write/read-back, GPT tail cleanup, exact-size disk and error detection'
} finally { Remove-Item -LiteralPath $work -Recurse -Force }
