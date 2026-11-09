# scripts/boot_switch.ps1's logic against a modelled firmware and disks (800-LNK-0037), as tests/boot_switch_test.py
# does for boot_switch.py: no firmware variable, disk or restart is touched.
#   powershell -ExecutionPolicy Bypass -File tests/boot_switch_ps1_test.ps1   (or pwsh -File ...)
$ErrorActionPreference = 'Stop'
$env:MIND_BOOT_SWITCH_STATE = Join-Path ([IO.Path]::GetTempPath()) 'mind-boot-switch-test.json'
. (Join-Path $PSScriptRoot '..\scripts\boot_switch.ps1') -Library
$script:failed = 0
function Check([string]$Name, $Got, $Want) {
    if ("$Got" -ceq "$Want") { [Console]::Out.WriteLine("ok   $Name") }
    else { [Console]::Out.WriteLine("FAIL ${Name}: wanted '$Want', got '$Got'"); $script:failed = 1 }
}
function Refusal([scriptblock]$Block) {
    try { & $Block | Out-Null; 'none' } catch { if ($_.Exception.GetType().Name -eq 'Refused') { $_.Exception.Message } else { throw } }
}
function Word([int]$Value) { , [BitConverter]::GetBytes([uint16]$Value) }
function Order([int[]]$Numbers) { $out = New-Object System.Collections.Generic.List[byte]; foreach ($n in $Numbers) { $out.AddRange([BitConverter]::GetBytes([uint16]$n)) }; , $out.ToArray() }
# A load option with arbitrary nodes before the HD node, as a firmware's own USB entries have, and an optional loader.
function Option([string]$Label, [byte[]]$Before, [uint32]$Signature, [uint64]$Start, [uint64]$Size, [int]$Format, [string]$File, [uint32]$Attributes = 1) {
    $hd = New-Object byte[] 42; $hd[0] = 4; $hd[1] = 1; $hd[2] = 42
    [BitConverter]::GetBytes([uint32]1).CopyTo($hd, 4); [BitConverter]::GetBytes($Start).CopyTo($hd, 8); [BitConverter]::GetBytes($Size).CopyTo($hd, 16)
    [BitConverter]::GetBytes($Signature).CopyTo($hd, 24); $hd[40] = $Format; $hd[41] = $Format
    $path = New-Object System.Collections.Generic.List[byte]; if ($Before) { $path.AddRange($Before) }; $path.AddRange($hd)
    if ($File) { $name = [Text.Encoding]::Unicode.GetBytes($File + [char]0); $path.AddRange([byte[]](4, 4, (4 + $name.Length), 0)); $path.AddRange($name) }
    $path.AddRange([byte[]](0x7F, 0xFF, 4, 0))
    $out = New-Object System.Collections.Generic.List[byte]
    $out.AddRange([BitConverter]::GetBytes($Attributes)); $out.AddRange([BitConverter]::GetBytes([uint16]$path.Count))
    $out.AddRange([Text.Encoding]::Unicode.GetBytes($Label + [char]0)); $out.AddRange($path.ToArray()); , $out.ToArray()
}
function Mbr([uint32]$Signature, [uint32]$Start, [uint32]$Size) {
    $s = New-Object byte[] 512; [BitConverter]::GetBytes($Signature).CopyTo($s, 440)
    [BitConverter]::GetBytes($Start).CopyTo($s, 454); [BitConverter]::GetBytes($Size).CopyTo($s, 458); $s[510] = 0x55; $s[511] = 0xAA; , $s
}
$SIG = [Convert]::ToUInt32('BE1AFDFA', 16); $START = [uint64]63; $SIZE = [uint64]1032129
# PciRoot(0x0)/Pci(0x14,0x0)/USB(3,0): what a firmware puts before the HD node of its own entry for a stick.
$USB = [byte[]](2, 1, 12, 0, 0xD0, 0x41, 3, 0x0A, 0, 0, 0, 0, 1, 1, 6, 0, 0, 0x14, 3, 5, 6, 0, 3, 0)

# The modelled machine: Windows' entry Boot0000 (GPT) first and booted, another stick's Boot0001, the MIND Core disk 2.
function New-Machine {
    $script:vars = @{ BootCurrent = (Word 0); BootOrder = (Order 0, 1)
        Boot0000 = (Option 'Windows Boot Manager' $null 0x11223344 2048 204800 2 '\EFI\Microsoft\Boot\bootmgfw.efi')
        Boot0001 = (Option 'UEFI: another stick' $USB 0x55667788 2048 1000 1 $null) }
    $script:writes = New-Object System.Collections.Generic.List[string]
    $script:out = New-Object System.Collections.Generic.List[string]
    $script:disks = @([pscustomobject]@{ Number = 1; Model = 'NVMe'; Serial = 'N'; Labels = @('', 'Windows') },
        [pscustomobject]@{ Number = 2; Model = 'USB stick'; Serial = 'S1'; Labels = @('MIND CORE', 'MIND LOG') })
    $script:sectors = @{ 2 = (Mbr $SIG $START $SIZE) }
    $script:logs = @('README.TXT', 'boot0003.log', 'HW0003.TXT'); $script:markBytes = [Text.Encoding]::ASCII.GetBytes("MIND-REACHED 1 7 HANDOVER`n")
    $script:saved = $null; $script:restarted = $false; $script:copiedTo = $null; $script:keepsOrder = $true
    @{
        Get = { param($name) if ($script:vars.ContainsKey($name)) { , ([byte[]]$script:vars[$name].Clone()) } else { $null } }
        Set = { param($name, $bytes) $script:writes.Add($name); $script:vars[$name] = [byte[]]$bytes
            if ($name -like 'Boot0*' -and -not $script:keepsOrder) { $script:vars['BootOrder'] = (Order 2, 0, 1) } }
        Disks = { $script:disks }
        Sector = { param($n) , $script:sectors[$n] }
        Read = { param($n, $p, $rel) if ($p -eq 1 -and $rel -eq $MARK -and $script:markBytes) { , $script:markBytes } else { $null } }
        Files = { param($n, $p) $script:logs }
        Copy = { param($n, $p, $to) $script:copiedTo = $to; $script:logs }
        Text = { param($path) 'boot log ... AUTORUN ENDED' }
        SecureBoot = { $false }
        Save = { param($text) $script:saved = $text }
        Load = { if ($null -eq $script:saved) { throw 'no state' }; $script:saved }
        Out = { param($text) $script:out.Add($text) }
        Restart = { $script:restarted = $true }
    }
}

# Parsing: a load option made here reads back, and the entry of the disk's first partition is found.
$made = ConvertFrom-LoadOption (ConvertTo-LoadOption 'MIND Core' $SIG $START $SIZE '\EFI\BOOT\BOOTX64.EFI')
Check 'a made load option reads back' ("{0} {1} {2} {3:X8} {4} {5} {6}" -f $made.Label, $made.Active, $made.Hd.Partition, $made.Hd.Signature, $made.Hd.Start, $made.Hd.Size, $made.File) 'MIND Core True 1 BE1AFDFA 63 1032129 \EFI\BOOT\BOOTX64.EFI'
$identity = Get-MbrIdentity (Mbr $SIG $START $SIZE)
$entries = @{ '0000' = (ConvertFrom-LoadOption (Option 'Windows' $null $SIG $START $SIZE 2 $null))
    '0001' = (ConvertFrom-LoadOption (Option 'other loader' $null $SIG $START $SIZE 1 '\EFI\other.efi'))
    '0002' = (ConvertFrom-LoadOption (Option 'other disk' $null 7 $START $SIZE 1 $null))
    '0003' = (ConvertFrom-LoadOption (Option 'UEFI: KINGSTON' $USB $SIG $START $SIZE 1 $null)) }
Check "the firmware's own entry of the stick, behind its USB nodes, is the disk's" (Find-Entry $entries $identity) '0003'
$entries.Remove('0003')
Check 'GPT, another loader or another disk are not' "$(Find-Entry $entries $identity)" ''
Check 'a sector without 55AA is no MBR' (Refusal { Get-MbrIdentity (New-Object byte[] 512) }) 'the disk has no MBR'
Check 'a truncated option is not read' "$(ConvertFrom-LoadOption ([byte[]](1, 0, 0, 0, 200, 0, 65, 0, 0, 0)))" ''

# once: an entry made, booted once; BootOrder never written.
$system = New-Machine
$number = Invoke-Once $system -1 $true
Check 'once makes the lowest free entry and boots it next' "$number $([BitConverter]::ToUInt16($script:vars['BootNext'], 0))" '0002 2'
Check 'what once writes, in order' ($script:writes -join ',') 'Boot0002,BootNext'
Check 'the made entry is HD(1,MBR,...)/File(BOOTX64)' ((ConvertFrom-LoadOption $script:vars['Boot0002']).File) '\EFI\BOOT\BOOTX64.EFI'
Check 'BootOrder stays Windows first' ((ConvertFrom-Json $script:saved).entry + ' ' + [BitConverter]::ToUInt16($script:vars['BootOrder'], 0)) '0002 0'
Check 'the state keeps the newest log and the mark' ("{0} {1} {2}" -f (ConvertFrom-Json $script:saved).newest_log, (ConvertFrom-Json $script:saved).mark, (ConvertFrom-Json $script:saved).disk) '3 7 2'
Check 'the restart is asked' $script:restarted 'True'
Check 'it says the next boot comes back to Windows' ($script:out -match 'every boot after it starts Boot0000').Count 1

# An existing inactive entry of the stick is activated and used, nothing made.
$system = New-Machine; $script:vars['Boot0005'] = (Option 'UEFI: KINGSTON' $USB $SIG $START $SIZE 1 $null 0)
$number = Invoke-Once $system -1 $false
Check 'an existing entry is activated and booted next' "$number $($script:writes -join ',') $((ConvertFrom-LoadOption $script:vars['Boot0005']).Active)" '0005 Boot0005,BootNext True'
Check 'no restart without -Reboot' $script:restarted 'False'

# Refusals change nothing.
$system = New-Machine; $script:vars['BootOrder'] = (Order 1, 0)
Check "Windows' entry not first is refused" ((Refusal { Invoke-Once $system -1 $false }) -replace ';.*', '') 'Boot0000, the entry Windows booted from, is not first in BootOrder (0001,0000)'
$system = New-Machine; $script:vars.Remove('BootCurrent')
Check 'no BootCurrent is refused' (Refusal { Invoke-Once $system -1 $false }) 'no BootCurrent: Windows was not started by UEFI'
$system = New-Machine; $script:disks = @($script:disks[0])
Check 'no MIND Core disk is refused' (Refusal { Invoke-Once $system -1 $false }) 'no disk with a MIND CORE volume followed by MIND LOG: write the image first'
$system = New-Machine; $script:disks = @()
Check 'no MBR disk at all is refused the same way' (Refusal { Invoke-Once $system -1 $false }) 'no disk with a MIND CORE volume followed by MIND LOG: write the image first'
$system = New-Machine; $script:disks += [pscustomobject]@{ Number = 3; Model = 'second'; Serial = 'S2'; Labels = @('MIND CORE', 'MIND LOG') }
Check 'two MIND Core disks are refused' (Refusal { Invoke-Once $system -1 $false }) 'several MIND Core disks: name one with -Disk (2, 3)'
Check 'a named disk without the volumes is refused' (Refusal { Invoke-Once $system 1 $false }) 'disk 1 has no MIND CORE volume followed by MIND LOG'
Check 'refusals wrote nothing' $script:writes.Count 0
$script:sectors[3] = (Mbr 9 63 1000); [void](Invoke-Once $system 3 $false)
Check 'with -Disk the named one is booted' ((ConvertFrom-LoadOption $script:vars['Boot0002']).Hd.Signature) 9

# A firmware that moved BootOrder when the entry was made stops before BootNext.
$system = New-Machine; $script:keepsOrder = $false
Check 'a moved BootOrder is refused' ((Refusal { Invoke-Once $system -1 $false }) -replace ';.*', '') 'the firmware changed BootOrder to 0002,0000,0001'
Check '... before BootNext' ($script:writes -join ',') 'Boot0002'

# logs: whether MIND Core booted since once, and else where the loader's mark says it stopped.
$system = New-Machine; [void](Invoke-Once $system -1 $false)
$script:logs = @('boot0003.log', 'boot0004.log', 'boot0005.log'); [void](Invoke-Logs $system 'C:\runs' -1)
Check 'logs say MIND Core booted' ($script:out | Where-Object { $_ -like 'MIND Core booted*' }) 'MIND Core booted: boot0004.log, boot0005.log; autorun in the last: ended'
Check 'logs copy into a new folder of DIR' ($script:copiedTo -like 'C:\runs\mind-log-*') 'True'
foreach ($case in @(@('MIND-REACHED 1 7 HANDOVER', 'the loader did not start'), @('MIND-REACHED 1 8 READ', 'boot 8: the loader read and checked'),
        @('MIND-REACHED 1 9 STARTED', '(2 boots since the switch'), @('MIND-REACHED 1 2 READ', 'went back from boot 7 to 2'))) {
    $system = New-Machine; [void](Invoke-Once $system -1 $false); $script:out.Clear()
    $script:markBytes = [Text.Encoding]::ASCII.GetBytes($case[0] + "`n"); [void](Invoke-Logs $system 'C:\runs' -1)
    Check "a mark of $($case[0])" (@($script:out | Where-Object { $_ -like 'MIND Core did not boot*' -and $_.Contains($case[1]) }).Count) 1
}
foreach ($bad in @('MIND-REACHED 2 7 READ', 'MIND-REACHED 1 7 ELSEWHERE', 'MIND-REACHED 1 x READ', 'MIND-REACHED 1 4294967296 READ', 'something else')) {
    Check "a mark of another shape is not read: $bad" "$(ConvertFrom-Mark ([Text.Encoding]::ASCII.GetBytes($bad)))" ''
}
$system = New-Machine; $script:markBytes = $null; [void](Invoke-Once $system -1 $false); $script:out.Clear(); [void](Invoke-Logs $system 'C:\runs' -1)
Check 'no mark at all is said' (@($script:out | Where-Object { $_ -like '*the loader left no mark*' }).Count) 1

# status changes nothing.
$system = New-Machine; Show-Status $system
Check 'status writes nothing' $script:writes.Count 0
Check 'status names the entries and the disk' (($script:out | Where-Object { $_ -match '^  Boot0000\* Windows Boot Manager \(Windows, booted now\)$|^MIND Core disk: disk 2 \(USB stick\), entry none yet' }).Count) 2
$system = New-Machine; $script:disks = @(); Show-Status $system
Check 'status without a stick says so' (@($script:out | Where-Object { $_ -like 'MIND Core disk: no disk with a MIND CORE volume*' }).Count) 1

# The machine's blocks run after New-WindowsSystem returns: a variable of its own is gone by then (the DEXP's logs, 2026-11-08).
$body = (Get-Command New-WindowsSystem).ScriptBlock.Ast.Body
$own = @($body.FindAll({ param($a) $a -is [Management.Automation.Language.AssignmentStatementAst] -and $a.Left -is [Management.Automation.Language.VariableExpressionAst] }, $false) | ForEach-Object { $_.Left.VariablePath.UserPath })
$blocks = @($body.FindAll({ param($a) $a -is [Management.Automation.Language.ScriptBlockExpressionAst] }, $false))
$used = @($blocks | ForEach-Object { $_.FindAll({ param($a) $a -is [Management.Automation.Language.VariableExpressionAst] }, $true) } | ForEach-Object { $_.VariablePath.UserPath } | Where-Object { $own -contains $_ } | Sort-Object -Unique)
Check "the machine's $($blocks.Count) blocks use none of New-WindowsSystem's variables" ($used -join ',') ''

if ($script:failed) { [Console]::Out.WriteLine('boot_switch.ps1: FAILED'); exit 1 }
[Console]::Out.WriteLine('boot_switch.ps1: all passed'); exit 0
