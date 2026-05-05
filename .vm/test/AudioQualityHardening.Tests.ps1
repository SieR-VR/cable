# Audio quality regression test for the Cable routing pipeline.
#
# Signal path under test:
#   C# WASAPI render (440 Hz sine) → driver render ring buffer
#   → VirtualAudioOutput (Cable runtime)
#   → VirtualAudioInput
#   → driver capture ring buffer → C# WASAPI capture
#
# Analysis:
#   1. C# in-memory: measure max consecutive silence window (10 ms RMS buckets).
#      Pass condition: max silence gap < 50 ms.
#   2. ffmpeg silencedetect (optional): parse WAV file written by C# probe.
#      Skipped when ffmpeg is unavailable; enable by passing -FfmpegExePath
#      to test.ps1.

BeforeAll {
    . (Join-Path $PSScriptRoot "common.ps1")
    $script:wasapiCs  = Get-CSharpLib "CableWasapi"
    $script:qualityCs = Get-CSharpLib "CableAudioQuality"
}

Describe "Audio quality: Cable routing signal continuity" {
    BeforeAll {
        $script:Session = Reset-Vm @VmContext
        Copy-GuestAppExe -Session $script:Session -ExePath $VmContext.AppExePath -ReuseVm $VmContext.ReuseVm
        Start-GuestHeadlessApp -Session $script:Session
        Invoke-AppRpc -Session $script:Session -Method POST -Path "/driver/connect" | Out-Null

        # Create one virtual render device (speaker) and one capture device (mic).
        $script:RenderDevice  = Invoke-AppRpc -Session $script:Session -Method POST `
            -Path "/virtual-devices" -Body @{ name = "QualityTestSpeaker"; deviceType = "render" }
        $script:CaptureDevice = Invoke-AppRpc -Session $script:Session -Method POST `
            -Path "/virtual-devices" -Body @{ name = "QualityTestMic"; deviceType = "capture" }

        $script:RenderDevice.endpointId  | Should -Not -BeNullOrEmpty
        $script:CaptureDevice.endpointId | Should -Not -BeNullOrEmpty

        # Build routing: VirtualAudioOutput(render) → VirtualAudioInput(capture).
        $graph = @{
            nodes = @(
                @{ type = "virtualAudioOutput"; data = @{ id = "n-render";  deviceId = $script:RenderDevice.id;  name = $script:RenderDevice.name  } },
                @{ type = "virtualAudioInput";  data = @{ id = "n-capture"; deviceId = $script:CaptureDevice.id; name = $script:CaptureDevice.name } }
            )
            edges = @(
                @{ id = "e1"; from = "n-render"; to = "n-capture" }
            )
        }
        Invoke-AppRpc -Session $script:Session -Method POST -Path "/graph"          -Body $graph | Out-Null
        Invoke-AppRpc -Session $script:Session -Method POST -Path "/runtime/enable"              | Out-Null
        Start-Sleep -Seconds 1

        # Copy ffmpeg to the guest if the host path is provided.
        if ($VmContext.FfmpegExePath -and (Test-Path $VmContext.FfmpegExePath)) {
            Install-GuestFfmpeg -Session $script:Session -HostFfmpegPath $VmContext.FfmpegExePath
        }
    }

    AfterAll {
        try   { Invoke-AppRpc -Session $script:Session -Method POST -Path "/runtime/disable"                                                    | Out-Null }
        catch { Write-Host "Cleanup warning (disable runtime): $_" }
        try   { Invoke-AppRpc -Session $script:Session -Method POST -Path "/graph" -Body @{ nodes = @(); edges = @() }                          | Out-Null }
        catch { Write-Host "Cleanup warning (empty graph): $_" }
        if ($script:CaptureDevice) {
            try   { Invoke-AppRpc -Session $script:Session -Method DELETE -Path "/virtual-devices/$($script:CaptureDevice.id)"                  | Out-Null }
            catch { Write-Host "Cleanup warning (delete capture): $_" }
        }
        if ($script:RenderDevice) {
            try   { Invoke-AppRpc -Session $script:Session -Method DELETE -Path "/virtual-devices/$($script:RenderDevice.id)"                   | Out-Null }
            catch { Write-Host "Cleanup warning (delete render): $_" }
        }
        Stop-GuestHeadlessApp -Session $script:Session
        if ($script:Session) {
            Assert-NoGuestBugCheck -ComputerName $VmContext.ComputerName -Port $VmContext.Port `
                -Username $VmContext.Username -Password $VmContext.Password `
                -Context "Audio quality hardening"
            Remove-PSSession $script:Session -ErrorAction SilentlyContinue
        }
    }

    It "renders a 5-second 440Hz tone through Cable without dropout gaps > 50ms" {
        $renderEndpointId  = $script:RenderDevice.endpointId
        $captureEndpointId = $script:CaptureDevice.endpointId

        $result = Invoke-GuestCSharpTest -Session $script:Session `
            -CSharpSources @($script:wasapiCs, $script:qualityCs) `
            -Script ([scriptblock]::Create(
                "[AudioQualityProbe]::Run('$renderEndpointId', '$captureEndpointId')"
            )) `
            -TempFileName "audio-quality-probe"

        $result | Should -Match '^QUALITY: '

        $maxSilenceMs = [double]($result -replace '^.*maxSilenceMs=([0-9.]+).*$', '$1')
        $maxSilenceMs | Should -BeLessThan 50 `
            -Because "dropout gaps over 50 ms indicate a routing failure or ring buffer underrun"
    }

    It "ffmpeg silencedetect reports no silence gaps >= 50ms in the captured tone" {
        # Determine whether ffmpeg is available inside the guest.
        $guestFfmpeg = Invoke-Command -Session $script:Session -ScriptBlock {
            if (Test-Path "C:\CableAudio\ffmpeg.exe") { "C:\CableAudio\ffmpeg.exe" }
            else { "" }
        }

        if (-not $guestFfmpeg) {
            Set-ItResult -Pending `
                -Because "ffmpeg not available in guest; pass -FfmpegExePath to test.ps1 to enable this test"
            return
        }

        # The WAV file must have been written by the previous It block.
        $wavPath    = "C:\CableAudio\quality-probe.wav"
        $wavPresent = Invoke-Command -Session $script:Session -ScriptBlock {
            param($p) Test-Path $p
        } -ArgumentList $wavPath
        $wavPresent | Should -BeTrue -Because "AudioQualityProbe must have written quality-probe.wav"

        # Run ffmpeg silencedetect. Analysis output goes to stderr; 2>&1 captures both.
        $output = Invoke-Command -Session $script:Session -ScriptBlock {
            param($ff, $wav)
            & $ff -y -i $wav -af "silencedetect=noise=-60dB:d=0.05" -f null NUL 2>&1
        } -ArgumentList $guestFfmpeg, $wavPath

        $outputStr = ($output -join "`n")

        # Extract all reported silence_duration values.
        $durations = [regex]::Matches($outputStr, 'silence_duration:\s*([\d.]+)') |
            ForEach-Object { [double]$_.Groups[1].Value }

        $longSilences = @($durations | Where-Object { $_ -ge 0.05 })
        $longSilences | Should -BeNullOrEmpty `
            -Because ("ffmpeg detected silence >= 50 ms: " + ($longSilences -join ", ") + " s")
    }
}
