<#
One boot of MIND Core from Windows by the firmware's BootNext, and that boot's logs read back (800-LNK-0037; as
boot_switch.py does from Linux, 000-KRN-0124).

  boot_switch.ps1 status                    the firmware's boot entries, Windows' and MIND Core's, the MIND Core disk
  boot_switch.ps1 once [-Disk N] [-Reboot]  BootNext set to the MIND Core disk's entry (made if missing)
  boot_switch.ps1 logs DIR [-Disk N]        the MIND LOG volume copied into DIR, and how far the last boot got by the
                                            loader's mark (000-KRN-0211)

BootOrder is never written: Windows' entry stays first, so whatever ends the run (reboot, a panic, the watchdog, a power
cut, a bootloader that refuses the volume) the next boot is Windows. Run as administrator, on a machine booted by UEFI:
  powershell -ExecutionPolicy Bypass -File scripts\boot_switch.ps1 status
#>
param(
    [Parameter(Position = 0)][string]$Command,
    [Parameter(Position = 1)][string]$Dir,
    [int]$Disk = -1,
    [switch]$Reboot,
    # Only defines the functions, for tests/boot_switch_ps1_test.ps1.
    [switch]$Library
)
Set-StrictMode -Version 3

$LOADER = '\EFI\BOOT\BOOTX64.EFI'
$LABEL = 'MIND Core'
$BOOT_VOLUME = 'MIND CORE'; $LOG_VOLUME = 'MIND LOG'
$STATE = if ($env:MIND_BOOT_SWITCH_STATE) { $env:MIND_BOOT_SWITCH_STATE } else { Join-Path $env:ProgramData 'mind-boot-switch.json' }
$LOG_NAME = '^boot(\d{4})\.log$'
# The loader's mark on the boot volume (bootloader/src/mark.rs): the boot count and the last point it reached.
$MARK = 'MIND\REACHED'
$STAGES = [ordered]@{
    'STARTED'  = 'the loader started and stopped before it had read and checked the kernel and the services'
    'READ'     = 'the loader read and checked the kernel and the services, and stopped before boot services ended (choosing the display, or starting the processors)'
    'HANDOVER' = "the loader handed over to the kernel: the boot stopped in the kernel, init or the services before the log volume's first write"
}
# Entries looked at: those in BootOrder and Boot0000 to Boot00FF; a new one takes the lowest free number there.
$ENTRIES_SCANNED = 0x100

class Refused : System.Exception { Refused([string]$Message) : base($Message) {} }

# An EFI_LOAD_OPTION (UEFI 2.x, 3.1.3): attributes, path length, description, device path; the HD and File nodes kept.
function ConvertFrom-LoadOption([byte[]]$Bytes) {
    if ($null -eq $Bytes -or $Bytes.Length -lt 8) { return $null }
    $attributes = [BitConverter]::ToUInt32($Bytes, 0); $pathLength = [BitConverter]::ToUInt16($Bytes, 4)
    $at = 6
    while ($at + 1 -lt $Bytes.Length -and ($Bytes[$at] -ne 0 -or $Bytes[$at + 1] -ne 0)) { $at += 2 }
    if ($at + 1 -ge $Bytes.Length) { return $null }
    $label = [Text.Encoding]::Unicode.GetString($Bytes, 6, $at - 6)
    $path = $at + 2; $end = $path + $pathLength
    if ($end -gt $Bytes.Length) { return $null }
    $hd = $null; $file = $null
    while ($path + 4 -le $end) {
        $type = $Bytes[$path]; $subtype = $Bytes[$path + 1]; $length = [BitConverter]::ToUInt16($Bytes, $path + 2)
        if ($length -lt 4 -or $path + $length -gt $end) { return $null }
        if ($type -eq 0x7F -and $subtype -eq 0xFF) { break }
        if ($type -eq 4 -and $subtype -eq 1 -and $length -eq 42) {
            $hd = [pscustomobject]@{
                Partition = [BitConverter]::ToUInt32($Bytes, $path + 4)
                Start = [BitConverter]::ToUInt64($Bytes, $path + 8); Size = [BitConverter]::ToUInt64($Bytes, $path + 16)
                Signature = [BitConverter]::ToUInt32($Bytes, $path + 24); Format = $Bytes[$path + 40]; SignatureType = $Bytes[$path + 41]
            }
        } elseif ($type -eq 4 -and $subtype -eq 4) {
            $file = [Text.Encoding]::Unicode.GetString($Bytes, $path + 4, $length - 4).TrimEnd([char]0)
        }
        $path += $length
    }
    [pscustomobject]@{ Active = ($attributes -band 1) -ne 0; Label = $label; Hd = $hd; File = $file; Attributes = $attributes }
}

# The load option boot_switch.py's efibootmgr --create-only makes: HD(1,MBR,signature,start,size)/File(loader).
function ConvertTo-LoadOption([string]$Label, [uint32]$Signature, [uint64]$Start, [uint64]$Size, [string]$Loader) {
    $hd = New-Object byte[] 42
    $hd[0] = 4; $hd[1] = 1; [BitConverter]::GetBytes([uint16]42).CopyTo($hd, 2)
    [BitConverter]::GetBytes([uint32]1).CopyTo($hd, 4); [BitConverter]::GetBytes($Start).CopyTo($hd, 8)
    [BitConverter]::GetBytes($Size).CopyTo($hd, 16); [BitConverter]::GetBytes($Signature).CopyTo($hd, 24)
    $hd[40] = 1; $hd[41] = 1
    $name = [Text.Encoding]::Unicode.GetBytes($Loader + [char]0)
    $file = New-Object byte[] (4 + $name.Length)
    $file[0] = 4; $file[1] = 4; [BitConverter]::GetBytes([uint16]$file.Length).CopyTo($file, 2); $name.CopyTo($file, 4)
    $endNode = [byte[]](0x7F, 0xFF, 4, 0)
    $description = [Text.Encoding]::Unicode.GetBytes($Label + [char]0)
    $pathLength = $hd.Length + $file.Length + $endNode.Length
    $out = New-Object System.Collections.Generic.List[byte]
    $out.AddRange([BitConverter]::GetBytes([uint32]1)); $out.AddRange([BitConverter]::GetBytes([uint16]$pathLength))
    $out.AddRange($description); $out.AddRange($hd); $out.AddRange($file); $out.AddRange($endNode)
    , $out.ToArray()
}

function Get-MbrIdentity([byte[]]$Sector) {
    if ($null -eq $Sector -or $Sector.Length -ne 512 -or $Sector[510] -ne 0x55 -or $Sector[511] -ne 0xAA) { throw [Refused]::new('the disk has no MBR') }
    [pscustomobject]@{ Signature = [BitConverter]::ToUInt32($Sector, 440)
        Start = [uint64][BitConverter]::ToUInt32($Sector, 454); Size = [uint64][BitConverter]::ToUInt32($Sector, 458) }
}

# The entry that starts the disk's first partition: HD(1,MBR,signature,start,size), with our loader or none.
function Find-Entry($Entries, $Identity) {
    foreach ($number in ($Entries.Keys | Sort-Object)) {
        $entry = $Entries[$number]; $hd = $entry.Hd
        if ($null -ne $hd -and $hd.Partition -eq 1 -and $hd.Format -eq 1 -and $hd.SignatureType -eq 1 -and $hd.Signature -eq $Identity.Signature `
                -and $hd.Start -eq $Identity.Start -and $hd.Size -eq $Identity.Size -and ($null -eq $entry.File -or $entry.File.ToUpper() -eq $LOADER)) {
            return $number
        }
    }
    $null
}

function Format-Number([int]$Number) { '{0:X4}' -f $Number }

# BootCurrent, BootNext, BootOrder and the entries, from the firmware's variables.
function Read-Boot($System) {
    $word = { param($bytes) if ($null -eq $bytes -or $bytes.Length -lt 2) { $null } else { Format-Number ([BitConverter]::ToUInt16($bytes, 0)) } }
    $orderBytes = & $System.Get 'BootOrder'
    $order = @(); if ($null -ne $orderBytes) { for ($i = 0; $i + 1 -lt $orderBytes.Length; $i += 2) { $order += Format-Number ([BitConverter]::ToUInt16($orderBytes, $i)) } }
    $entries = @{}
    $numbers = @($order) + @(0..($ENTRIES_SCANNED - 1) | ForEach-Object { Format-Number $_ }) | Select-Object -Unique
    foreach ($number in $numbers) {
        $option = ConvertFrom-LoadOption (& $System.Get ('Boot' + $number))
        if ($null -ne $option) { $entries[$number] = $option }
    }
    [pscustomobject]@{ Current = & $word (& $System.Get 'BootCurrent'); Next = & $word (& $System.Get 'BootNext'); Order = $order; Entries = $entries }
}

# Windows, the entry this system booted from, must come first in BootOrder.
function Test-Order($Boot) {
    if ($null -eq $Boot.Current) { throw [Refused]::new('no BootCurrent: Windows was not started by UEFI') }
    if ($Boot.Order.Count -eq 0 -or $Boot.Order[0] -ne $Boot.Current) {
        $order = if ($Boot.Order.Count) { $Boot.Order -join ',' } else { 'empty' }
        throw [Refused]::new("Boot$($Boot.Current), the entry Windows booted from, is not first in BootOrder ($order); a restart after MIND Core would not come back here. Put it first in the firmware's setup")
    }
}

# Disks whose first partition is the MIND CORE volume and whose second is MIND LOG; the one named, or the only one.
function Select-MindDisk($Disks, [int]$Requested) {
    # No MBR disk at all binds $null, which a pipeline would still pass once (the DEXP, 2026-11-08).
    $found = @(foreach ($d in @($Disks)) { if ($null -ne $d -and $d.Labels.Count -ge 2 -and $d.Labels[0] -eq $BOOT_VOLUME -and $d.Labels[1] -eq $LOG_VOLUME) { $d } })
    if ($Requested -ge 0) {
        $found = @($found | Where-Object { $_.Number -eq $Requested })
        if ($found.Count -eq 0) { throw [Refused]::new("disk $Requested has no $BOOT_VOLUME volume followed by $LOG_VOLUME") }
    }
    if ($found.Count -eq 0) { throw [Refused]::new("no disk with a $BOOT_VOLUME volume followed by ${LOG_VOLUME}: write the image first") }
    if ($found.Count -gt 1) { throw [Refused]::new('several MIND Core disks: name one with -Disk (' + (($found | ForEach-Object { $_.Number }) -join ', ') + ')') }
    $found[0]
}

# (count, stage) of the loader's mark, or $null for one of another shape.
function ConvertFrom-Mark([byte[]]$Data) {
    if ($null -eq $Data) { return $null }
    $line = ([Text.Encoding]::ASCII.GetString($Data) -split "`n", 2)[0]
    $words = @($line.Trim() -split '\s+' | Where-Object { $_ })
    if ($words.Count -ne 4 -or $words[0] -ne 'MIND-REACHED' -or $words[1] -ne '1' -or $words[2] -notmatch '^[0-9]+$') { return $null }
    $count = [uint64]$words[2]
    if (-not $STAGES.Contains($words[3]) -or $count -ge [uint64]4294967296) { return $null }
    , @([int64]$count, $words[3])
}

# What the loader's mark says of the boots since `once` recorded $Before, for a run that left no new log.
function Get-Reached($Before, $Mark) {
    if ($null -eq $Mark) { return 'the loader left no mark: it never started, or it is older than the mark (000-KRN-0211)' }
    $count = $Mark[0]; $stage = $Mark[1]
    if ($null -eq $Before) { return "the loader's mark is boot $count, $stage; no mark was recorded at the switch to compare it with" }
    if ($count -eq $Before) { return "the loader's mark is still boot ${count}: the loader did not start. The firmware may skip USB (Fast Boot), ignore BootNext, or stop before it starts the stick's loader" }
    if ($count -lt $Before) { return "the loader's mark went back from boot $Before to ${count}: the stick was written again since" }
    $since = if ($count - $Before -gt 1) { " ($($count - $Before) boots since the switch; the mark is the last one's)" } else { '' }
    "boot ${count}: $($STAGES[$stage])$since"
}

function Get-NewestLog($Names) {
    $newest = 0
    foreach ($name in $Names) { if ($name -match $LOG_NAME) { $newest = [Math]::Max($newest, [int]$Matches[1]) } }
    $newest
}

function Show-Status($System) {
    $boot = Read-Boot $System
    $order = if ($boot.Order.Count) { $boot.Order -join ',' } else { '-' }
    & $System.Out ("BootCurrent {0}, BootNext {1}, BootOrder {2}" -f $(if ($boot.Current) { $boot.Current } else { '-' }), $(if ($boot.Next) { $boot.Next } else { '-' }), $order)
    foreach ($number in ($boot.Entries.Keys | Sort-Object)) {
        $entry = $boot.Entries[$number]; $mark = if ($number -eq $boot.Current) { ' (Windows, booted now)' } else { '' }
        & $System.Out ("  Boot{0}{1} {2}{3}" -f $number, $(if ($entry.Active) { '*' } else { ' ' }), $entry.Label, $mark)
    }
    try {
        $disk = Select-MindDisk (& $System.Disks) -1
        $number = Find-Entry $boot.Entries (Get-MbrIdentity (& $System.Sector $disk.Number))
        & $System.Out ("MIND Core disk: disk {0} ({1}), entry {2}" -f $disk.Number, $disk.Model, $(if ($number) { 'Boot' + $number } else { 'none yet (once makes one)' }))
    } catch [Refused] { & $System.Out "MIND Core disk: $($_.Exception.Message)" }
    if (& $System.SecureBoot) { & $System.Out 'Secure Boot is on: MIND Core starts only if its bootloader is signed with a key the firmware trusts' }
}

function Invoke-Once($System, [int]$Requested, [bool]$Restart) {
    $boot = Read-Boot $System
    Test-Order $boot
    $disk = Select-MindDisk (& $System.Disks) $Requested
    $identity = Get-MbrIdentity (& $System.Sector $disk.Number)
    $number = Find-Entry $boot.Entries $identity
    if ($null -eq $number) {
        $free = 0..($ENTRIES_SCANNED - 1) | ForEach-Object { Format-Number $_ } | Where-Object { -not $boot.Entries.ContainsKey($_) -and $boot.Order -notcontains $_ } | Select-Object -First 1
        if ($null -eq $free) { throw [Refused]::new('no free entry number from Boot0000 to Boot00FF') }
        & $System.Set ('Boot' + $free) (ConvertTo-LoadOption $LABEL $identity.Signature $identity.Start $identity.Size $LOADER)
        $after = Read-Boot $System
        if (($after.Order -join ',') -ne ($boot.Order -join ',')) {
            throw [Refused]::new("the firmware changed BootOrder to $($after.Order -join ','); put back $($boot.Order -join ',') in its setup before switching")
        }
        $number = Find-Entry $after.Entries $identity
        if ($null -eq $number) { throw [Refused]::new("the new boot entry does not name the disk's first partition") }
        & $System.Out "made Boot$number $LABEL for disk $($disk.Number)"
        $boot = $after
    }
    $entry = $boot.Entries[$number]
    if (-not $entry.Active) {
        $bytes = & $System.Get ('Boot' + $number); $bytes[0] = $bytes[0] -bor 1; & $System.Set ('Boot' + $number) $bytes
    }
    $newest = Get-NewestLog (& $System.Files $disk.Number 2)
    $mark = ConvertFrom-Mark (& $System.Read $disk.Number 1 $MARK)
    & $System.Set 'BootNext' ([BitConverter]::GetBytes([uint16][Convert]::ToUInt16($number, 16)))
    if ((Read-Boot $System).Next -ne $number) { throw [Refused]::new('the firmware did not keep BootNext') }
    $state = [ordered]@{ disk = $disk.Number; serial = $disk.Serial; entry = $number; newest_log = $newest
        mark = $(if ($mark) { $mark[0] } else { $null }); time = [int64]([DateTimeOffset]::UtcNow.ToUnixTimeSeconds()) }
    & $System.Save ($state | ConvertTo-Json -Compress)
    & $System.Out "the next boot starts Boot$number (disk $($disk.Number)) once; every boot after it starts Boot$($boot.Order[0])"
    if (& $System.SecureBoot) { & $System.Out "Secure Boot is on: if the firmware refuses MIND Core's bootloader, it starts Windows instead" }
    & $System.Out "With the firmware's Fast Boot on, a firmware may skip the stick and BootNext and start Windows (the Ryzen did): turn Fast Boot off in its setup"
    & $System.Out "Windows mounts the stick's volumes and may write to them (System Volume Information): restart with shutdown /r, not the power button"
    if ($Restart) { & $System.Out 'restarting in 5 s'; & $System.Restart }
    $number
}

function Invoke-Logs($System, [string]$Target, [int]$Requested) {
    $disk = Select-MindDisk (& $System.Disks) $Requested
    $before = $null; $beforeMark = $null
    try { $state = (& $System.Load) | ConvertFrom-Json; $before = $state.newest_log; $beforeMark = $state.mark } catch { }
    $destination = Join-Path $Target ('mind-log-' + (Get-Date -Format 'yyyyMMdd-HHmmss'))
    $names = & $System.Copy $disk.Number 2 $destination
    $newest = Get-NewestLog $names
    $mark = ConvertFrom-Mark (& $System.Read $disk.Number 1 $MARK)
    & $System.Out "copied the MIND LOG volume of disk $($disk.Number) into $destination"
    if ($null -eq $before) {
        & $System.Out ('the newest boot log is boot{0:D4}.log (no switch was recorded)' -f $newest)
    } elseif ($newest -gt $before) {
        $boots = (($before + 1)..$newest | ForEach-Object { 'boot{0:D4}.log' -f $_ }) -join ', '
        $name = $names | Where-Object { $_ -match $LOG_NAME -and [int]$Matches[1] -eq $newest } | Select-Object -First 1
        $text = & $System.Text (Join-Path $destination $name)
        $autorun = if ($text -match 'AUTORUN ENDED') { 'ended' } elseif ($text -match 'AUTORUN') { 'started, no end logged' } else { 'none' }
        & $System.Out "MIND Core booted: $boots; autorun in the last: $autorun"
        if ($mark) { & $System.Out "the loader's mark: boot $($mark[0]), $($mark[1])" }
    } else {
        & $System.Out "MIND Core did not boot: no new boot log; $(Get-Reached $beforeMark $mark)"
    }
    $destination
}

# A partition's volume: its letter, or its \\?\Volume{...}\ path without one. A function, as the blocks below run after New-WindowsSystem returns.
function Get-VolumeRoot([int]$Number, [int]$Partition) {
    $volume = Get-Partition -DiskNumber $Number -PartitionNumber $Partition | Get-Volume
    if ($volume.DriveLetter) { "$($volume.DriveLetter):\" } else { $volume.Path }
}

# What the script does to the machine; the tests replace it.
function New-WindowsSystem {
    if (-not ('MindFirmware' -as [type])) {
        Add-Type -TypeDefinition @'
using System; using System.ComponentModel; using System.Runtime.InteropServices;
public static class MindFirmware {
    const string Global = "{8BE4DF61-93CA-11D2-AA0D-00E098032B8C}";
    [DllImport("kernel32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
    static extern uint GetFirmwareEnvironmentVariableExW(string name, string guid, byte[] buffer, uint size, out uint attributes);
    [DllImport("kernel32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
    static extern bool SetFirmwareEnvironmentVariableExW(string name, string guid, byte[] value, uint size, uint attributes);
    [DllImport("advapi32.dll", SetLastError = true)] static extern bool OpenProcessToken(IntPtr process, uint access, out IntPtr token);
    [DllImport("advapi32.dll", SetLastError = true, CharSet = CharSet.Unicode)] static extern bool LookupPrivilegeValueW(string system, string name, out long luid);
    [StructLayout(LayoutKind.Sequential, Pack = 1)] struct Privilege { public int Count; public long Luid; public int Attributes; }
    [DllImport("advapi32.dll", SetLastError = true)] static extern bool AdjustTokenPrivileges(IntPtr token, bool none, ref Privilege state, int length, IntPtr previous, IntPtr returned);
    [DllImport("kernel32.dll")] static extern IntPtr GetCurrentProcess();
    // SeSystemEnvironmentPrivilege, which administrators hold but have to enable.
    public static void Enable() {
        IntPtr token; long luid;
        if (!OpenProcessToken(GetCurrentProcess(), 0x28, out token) || !LookupPrivilegeValueW(null, "SeSystemEnvironmentPrivilege", out luid)) throw new Win32Exception(Marshal.GetLastWin32Error());
        var state = new Privilege { Count = 1, Luid = luid, Attributes = 2 };
        if (!AdjustTokenPrivileges(token, false, ref state, 0, IntPtr.Zero, IntPtr.Zero) || Marshal.GetLastWin32Error() == 1300) throw new Win32Exception(1300, "not an administrator: the firmware's variables need SeSystemEnvironmentPrivilege");
    }
    public static byte[] Get(string name) {
        var buffer = new byte[4096]; uint attributes;
        uint length = GetFirmwareEnvironmentVariableExW(name, Global, buffer, (uint)buffer.Length, out attributes);
        if (length == 0) { int error = Marshal.GetLastWin32Error(); if (error == 203) return null; throw new Win32Exception(error); }
        Array.Resize(ref buffer, (int)length); return buffer;
    }
    // Non-volatile, boot service and runtime access, as the firmware's own boot variables.
    public static void Set(string name, byte[] value) {
        if (!SetFirmwareEnvironmentVariableExW(name, Global, value, (uint)value.Length, 7)) throw new Win32Exception(Marshal.GetLastWin32Error());
    }
}
'@
    }
    [MindFirmware]::Enable()
    @{
        Get = { param($name) [MindFirmware]::Get($name) }
        Set = { param($name, $bytes) [MindFirmware]::Set($name, [byte[]]$bytes) }
        Disks = {
            foreach ($d in Get-Disk | Where-Object { $_.PartitionStyle -eq 'MBR' }) {
                $labels = @(Get-Partition -DiskNumber $d.Number -ErrorAction SilentlyContinue | Sort-Object PartitionNumber |
                    ForEach-Object { $v = $_ | Get-Volume -ErrorAction SilentlyContinue; if ($v) { $v.FileSystemLabel } else { '' } })
                [pscustomobject]@{ Number = $d.Number; Model = $d.FriendlyName; Serial = $d.SerialNumber; Labels = $labels }
            }
        }
        Sector = {
            param([int]$number)
            $stream = New-Object IO.FileStream ("\\.\PhysicalDrive$number"), 'Open', 'Read', 'ReadWrite'
            try { $sector = New-Object byte[] 512; [void]$stream.Read($sector, 0, 512); , $sector } finally { $stream.Dispose() }
        }
        Read = {
            param([int]$number, [int]$partition, [string]$relative)
            $file = [IO.Path]::Combine((Get-VolumeRoot $number $partition), $relative)
            if ([IO.File]::Exists($file)) { , [IO.File]::ReadAllBytes($file) } else { $null }
        }
        Files = { param([int]$number, [int]$partition) @([IO.Directory]::GetFiles((Get-VolumeRoot $number $partition)) | ForEach-Object { [IO.Path]::GetFileName($_) }) }
        # Windows' own folders on the volume are left out: they are not MIND Core's, and reading them is refused. .NET
        # rather than Copy-Item, which takes no \\?\Volume{...}\ path of a volume without a letter.
        Copy = {
            param([int]$number, [int]$partition, [string]$destination)
            $copy = {
                param($from, $to)
                [void][IO.Directory]::CreateDirectory($to)
                foreach ($file in [IO.Directory]::GetFiles($from)) { [IO.File]::Copy($file, [IO.Path]::Combine($to, [IO.Path]::GetFileName($file))) }
                foreach ($folder in [IO.Directory]::GetDirectories($from)) {
                    $name = [IO.Path]::GetFileName($folder)
                    if (@('System Volume Information', '$RECYCLE.BIN') -notcontains $name) { & $copy $folder ([IO.Path]::Combine($to, $name)) }
                }
            }
            & $copy (Get-VolumeRoot $number $partition) $destination
            @([IO.Directory]::GetFiles($destination) | ForEach-Object { [IO.Path]::GetFileName($_) })
        }
        Text = { param($path) [IO.File]::ReadAllText($path) }
        Out = { param($text) [Console]::Out.WriteLine($text) }
        SecureBoot = { try { Confirm-SecureBootUEFI } catch { $null } }
        Save = { param($text) Set-Content -LiteralPath $STATE -Value $text -Encoding ASCII }
        Load = { Get-Content -LiteralPath $STATE -Raw }
        # In 5 s, by Windows' own shutdown: the restart outlives the caller, an Effector exec among them.
        Restart = { & shutdown.exe /r /t 5 /c 'MIND Core once (boot_switch.ps1)' }
    }
}

if ($Library) { return }
try {
    switch ($Command) {
        'status' { Show-Status (New-WindowsSystem) }
        'once' { [void](Invoke-Once (New-WindowsSystem) $Disk $Reboot.IsPresent) }
        'logs' {
            if (-not $Dir) { throw [Refused]::new('logs needs DIR') }
            [void](Invoke-Logs (New-WindowsSystem) $Dir $Disk)
        }
        default { [Console]::Out.WriteLine('usage: boot_switch.ps1 status | once [-Disk N] [-Reboot] | logs DIR [-Disk N]'); exit 2 }
    }
} catch [Refused] {
    [Console]::Error.WriteLine("boot_switch: refused: $($_.Exception.Message)"); exit 1
} catch {
    [Console]::Error.WriteLine("boot_switch: $($_.Exception.Message)"); exit 1
}
exit 0
