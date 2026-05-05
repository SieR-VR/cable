# IOCTL create/remove tests via headless REST API.
# Replaces the previous C# IOCTL-based test: virtual device lifecycle is now
# exercised through the app's HTTP RPC server, which mirrors the production
# code path used by the GUI.

BeforeAll {
    . (Join-Path $PSScriptRoot "common.ps1")
}

Describe "IOCTL: device create/remove" {
    BeforeAll {
        $script:Session = Reset-Vm @VmContext
        Copy-GuestAppExe -Session $script:Session -ExePath $VmContext.AppExePath -ReuseVm $VmContext.ReuseVm
        Start-GuestHeadlessApp -Session $script:Session
        Invoke-AppRpc -Session $script:Session -Method POST -Path "/driver/connect" | Out-Null
    }

    AfterAll {
        Stop-GuestHeadlessApp -Session $script:Session
        if ($script:Session) {
            Assert-NoGuestBugCheck -ComputerName $VmContext.ComputerName -Port $VmContext.Port -Username $VmContext.Username -Password $VmContext.Password -Context "IOCTL device create/remove"
            Remove-PSSession $script:Session -ErrorAction SilentlyContinue
        }
    }

    It "creates a capture (mic) device and removes it cleanly" {
        $device = Invoke-AppRpc -Session $script:Session -Method POST -Path "/virtual-devices" `
            -Body @{ name = "IOCTL Mic Test"; deviceType = "capture" }

        $device | Should -Not -BeNullOrEmpty
        $device.id | Should -Not -BeNullOrEmpty
        $device.deviceType | Should -Be "capture"

        Invoke-AppRpc -Session $script:Session -Method DELETE -Path "/virtual-devices/$($device.id)" | Out-Null
    }

    It "creates a render (speaker) device and removes it cleanly" {
        $device = Invoke-AppRpc -Session $script:Session -Method POST -Path "/virtual-devices" `
            -Body @{ name = "IOCTL Speaker Test"; deviceType = "render" }

        $device | Should -Not -BeNullOrEmpty
        $device.id | Should -Not -BeNullOrEmpty
        $device.deviceType | Should -Be "render"

        Invoke-AppRpc -Session $script:Session -Method DELETE -Path "/virtual-devices/$($device.id)" | Out-Null
    }
}
