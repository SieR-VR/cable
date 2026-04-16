# IOCTL test suite for CableAudio driver.
# Both capture and render create/remove are tested in one VM session since each
# It block performs a clean create→remove cycle with no leftover state.

BeforeAll {
    . (Join-Path $PSScriptRoot "common.ps1")
    $script:ioctlCs = Get-CSharpLib "CableIoctl"

    $script:ioctlHelpers = @'
function Run-CreateRemove {
    param([int]$DeviceType, [string]$Name)
    $create = [CableIoctl]::Create($DeviceType, $Name)
    if ($create -notlike "CREATE OK: Id=*") { return @($create) }
    $id = [CableIoctl]::ParseCreateId($create)
    $remove = [CableIoctl]::Remove($id)
    return @($create, $remove)
}
'@
}

Describe "IOCTL: device create/remove" {
    BeforeAll {
        $script:Session = Reset-Vm @VmContext
    }

    AfterAll {
        if ($script:Session) {
            Assert-NoGuestBugCheck -ComputerName $VmContext.ComputerName -Port $VmContext.Port -Username $VmContext.Username -Password $VmContext.Password -Context "IOCTL device create/remove"
            Remove-PSSession $script:Session -ErrorAction SilentlyContinue
        }
    }

    It "creates a capture (mic) device and removes it cleanly" {
        $output = Invoke-GuestCSharpTest -Session $script:Session `
            -CSharpSources @($script:ioctlCs) `
            -HelperFunctions $script:ioctlHelpers `
            -Script {
                $results = @()
                $results += Run-CreateRemove -DeviceType 1 -Name "IOCTL Mic Test"
                $results
            } `
            -TempFileName "ioctl-suite"

        $output | Should -Not -Match 'FAILED|ERR:'
        $output | Should -Contain "REMOVE OK"
    }

    It "creates a render (speaker) device and removes it cleanly" {
        $output = Invoke-GuestCSharpTest -Session $script:Session `
            -CSharpSources @($script:ioctlCs) `
            -HelperFunctions $script:ioctlHelpers `
            -Script {
                $results = @()
                $results += Run-CreateRemove -DeviceType 0 -Name "IOCTL Speaker Test"
                $results
            } `
            -TempFileName "ioctl-suite"

        $output | Should -Not -Match 'FAILED|ERR:'
        $output | Should -Contain "REMOVE OK"
    }
}
