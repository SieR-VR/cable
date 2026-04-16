# IOCTL hardening regression tests for CableAudio driver.
# Verifies STATUS_DEVICE_BUSY behavior and map/unmap/remove stability loops.

BeforeAll {
    . (Join-Path $PSScriptRoot "common.ps1")
    $script:ioctlCs = Get-CSharpLib "CableIoctl"

    $script:hardeningHelpers = @'
function Parse-CreateId {
    param([string]$CreateResult)
    return [CableIoctl]::ParseCreateId($CreateResult)
}

function Parse-MapAddress {
    param([string]$MapResult)
    return [CableIoctl]::ParseMapAddress($MapResult)
}

function Invoke-BusyRemoveSequence {
    param(
        [int]$DeviceType,
        [string]$Name
    )

    $log = @()
    $create = [CableIoctl]::Create($DeviceType, $Name)
    $log += $create

    $id = Parse-CreateId -CreateResult $create

    $map = [CableIoctl]::MapRingBuffer($id)
    $log += $map

    $removeBusy = [CableIoctl]::Remove($id)
    $log += $removeBusy

    $address = Parse-MapAddress -MapResult $map
    $unmap = [CableIoctl]::UnmapRingBuffer($id, $address)
    $log += $unmap

    $removeFinal = [CableIoctl]::Remove($id)
    $log += $removeFinal

    return $log
}

function Invoke-WrongAddressUnmapSequence {
    param(
        [int]$DeviceType,
        [string]$Name
    )

    $log = @()
    $create = [CableIoctl]::Create($DeviceType, $Name)
    $log += $create

    $id = Parse-CreateId -CreateResult $create

    $map = [CableIoctl]::MapRingBuffer($id)
    $log += $map

    $address = Parse-MapAddress -MapResult $map
    $wrongAddress = $address + 0x1000

    $wrongUnmap = [CableIoctl]::UnmapRingBuffer($id, $wrongAddress)
    $log += $wrongUnmap

    $validUnmap = [CableIoctl]::UnmapRingBuffer($id, $address)
    $log += $validUnmap

    $remove = [CableIoctl]::Remove($id)
    $log += $remove

    return $log
}
'@
}

Describe "IOCTL: hardening" {
    BeforeAll {
        $script:Session = Reset-Vm @VmContext
    }

    AfterAll {
        if ($script:Session) {
            Assert-NoGuestBugCheck -ComputerName $VmContext.ComputerName -Port $VmContext.Port -Username $VmContext.Username -Password $VmContext.Password -Context "IOCTL hardening"
            Remove-PSSession $script:Session -ErrorAction SilentlyContinue
        }
    }

    It "returns busy while mapped and succeeds after unmap" {
        $output = Invoke-GuestCSharpTest -Session $script:Session `
            -CSharpSources @($script:ioctlCs) `
            -HelperFunctions $script:hardeningHelpers `
            -Script {
                Invoke-BusyRemoveSequence -DeviceType 0 -Name "Hardening Busy Remove"
            } `
            -TempFileName "ioctl-hardening-suite"

        $output | Should -Contain "UNMAP OK"
        $output | Should -Contain "REMOVE OK"
        ($output -join "`n") | Should -Match 'REMOVE FAILED: error=(170|2404)'
    }

    It "repeats busy-remove then unmap-remove cycles" {
        $loopCount = if ($VmContext.ContainsKey('RenameLoopCount')) {
            [Math]::Max(2, [int]$VmContext.RenameLoopCount)
        } else {
            3
        }

        $guestScript = @"
`$results = @()
`$loopCount = $loopCount

for (`$i = 0; `$i -lt `$loopCount; `$i++) {
    `$results += "LOOP `$i"
    `$results += Invoke-BusyRemoveSequence -DeviceType 1 -Name ("Hardening Stress " + `$i)
}

`$results
"@

        $output = Invoke-GuestCSharpTest -Session $script:Session `
            -CSharpSources @($script:ioctlCs) `
            -HelperFunctions $script:hardeningHelpers `
            -Script ([scriptblock]::Create($guestScript)) `
            -TempFileName "ioctl-hardening-suite"

        ($output | Where-Object { $_ -eq 'REMOVE OK' }).Count | Should -BeGreaterThan 0
        ($output | Where-Object { $_ -eq 'UNMAP OK' }).Count | Should -BeGreaterThan 0
        ($output -join "`n") | Should -Not -Match 'ERR:OpenDriver|CREATE FAILED|MAP FAILED|UNMAP FAILED'
    }

    It "rejects wrong unmap address and still allows valid cleanup" {
        $output = Invoke-GuestCSharpTest -Session $script:Session `
            -CSharpSources @($script:ioctlCs) `
            -HelperFunctions $script:hardeningHelpers `
            -Script {
                Invoke-WrongAddressUnmapSequence -DeviceType 0 -Name "Hardening Wrong Unmap"
            } `
            -TempFileName "ioctl-hardening-suite"

        ($output -join "`n") | Should -Match 'UNMAP FAILED: error=87'
        $output | Should -Contain "UNMAP OK"
        $output | Should -Contain "REMOVE OK"
    }
}
