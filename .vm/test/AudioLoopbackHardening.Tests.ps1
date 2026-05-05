# Audio loopback regression test for virtual Cable endpoints.
# Goal: verify that audio written to a virtual render endpoint travels through
# the Cable runtime and is captured by the virtual microphone endpoint.
#
# Signal path:
#   C# WASAPI render → driver render ring buffer
#   → VirtualAudioOutput (Cable runtime)
#   → VirtualAudioInput
#   → driver capture ring buffer → C# WASAPI capture
#
# The headless app serves both the device lifecycle (REST API) and the audio
# runtime (POST /runtime/enable), giving this test real end-to-end coverage.

BeforeAll {
    . (Join-Path $PSScriptRoot "common.ps1")
    $script:wasapiCs = Get-CSharpLib "CableWasapi"

    # C# helper: given the endpoint IDs of a virtual speaker and virtual mic
    # already created and routed by the Cable runtime, render a short burst of
    # non-zero audio and return how many absolute-deviation bytes were captured.
    $script:loopbackCs = @'
public static class CableAudioLoopbackProbe
{
    static void FillRenderBuffer(IAudioRenderClient renderClient, uint frames, WAVEFORMATEX format)
    {
        IntPtr pData;
        CableWasapi.ThrowIfFailed(renderClient.GetBuffer(frames, out pData), "IAudioRenderClient::GetBuffer");
        int bytes = checked((int)(frames * format.nBlockAlign));

        byte[] block = new byte[bytes];
        int frameStride = Math.Max(1, (int)format.nBlockAlign);
        for (int i = 0; i < bytes; i++)
        {
            block[i] = (byte)((i / frameStride) % 2 == 0 ? 0x10 : 0xF0);
        }

        Marshal.Copy(block, 0, pData, bytes);
        CableWasapi.ThrowIfFailed(renderClient.ReleaseBuffer(frames, 0), "IAudioRenderClient::ReleaseBuffer");
    }

    static long CaptureSignalBytes(IAudioCaptureClient captureClient, WAVEFORMATEX format)
    {
        long totalAbs = 0;
        uint nextPacket;
        CableWasapi.ThrowIfFailed(captureClient.GetNextPacketSize(out nextPacket), "GetNextPacketSize");
        while (nextPacket > 0)
        {
            IntPtr pData;
            uint frames, flags;
            ulong pos, qpc;
            CableWasapi.ThrowIfFailed(captureClient.GetBuffer(out pData, out frames, out flags, out pos, out qpc), "GetBuffer");
            int bytes = checked((int)(frames * format.nBlockAlign));
            if (bytes > 0)
            {
                byte[] buf = new byte[bytes];
                Marshal.Copy(pData, buf, 0, bytes);
                for (int i = 0; i < buf.Length; i++)
                    totalAbs += Math.Abs((int)buf[i] - 128);
            }
            CableWasapi.ThrowIfFailed(captureClient.ReleaseBuffer(frames), "ReleaseBuffer");
            CableWasapi.ThrowIfFailed(captureClient.GetNextPacketSize(out nextPacket), "GetNextPacketSize(loop)");
        }
        return totalAbs;
    }

    // renderEndpointId / captureEndpointId come from the REST API response.
    public static string Run(string renderEndpointId, string captureEndpointId)
    {
        var enumerator = (IMMDeviceEnumerator)Activator.CreateInstance(
            Type.GetTypeFromCLSID(CableWasapi.CLSID_MMDeviceEnumerator));

        IMMDevice renderDev, captureDev;
        CableWasapi.ThrowIfFailed(enumerator.GetDevice(renderEndpointId,  out renderDev),  "GetDevice(render)");
        CableWasapi.ThrowIfFailed(enumerator.GetDevice(captureEndpointId, out captureDev), "GetDevice(capture)");

        var renderClient  = CableWasapi.ActivateAudioClient(renderDev);
        var captureClient = CableWasapi.ActivateAudioClient(captureDev);

        IntPtr pRenderFormat  = CableWasapi.GetMixFormatPtr(renderClient);
        IntPtr pCaptureFormat = CableWasapi.GetMixFormatPtr(captureClient);
        WAVEFORMATEX renderFormat  = (WAVEFORMATEX)Marshal.PtrToStructure(pRenderFormat,  typeof(WAVEFORMATEX));
        WAVEFORMATEX captureFormat = (WAVEFORMATEX)Marshal.PtrToStructure(pCaptureFormat, typeof(WAVEFORMATEX));

        long hnsBuffer = 2000000;
        CableWasapi.ThrowIfFailed(renderClient.Initialize( CableWasapi.AUDCLNT_SHAREMODE_SHARED, 0, hnsBuffer, 0, pRenderFormat,  IntPtr.Zero), "Render Initialize");
        CableWasapi.ThrowIfFailed(captureClient.Initialize(CableWasapi.AUDCLNT_SHAREMODE_SHARED, 0, hnsBuffer, 0, pCaptureFormat, IntPtr.Zero), "Capture Initialize");

        uint renderBufferFrames, captureBufferFrames;
        CableWasapi.ThrowIfFailed(renderClient.GetBufferSize( out renderBufferFrames),  "Render GetBufferSize");
        CableWasapi.ThrowIfFailed(captureClient.GetBufferSize(out captureBufferFrames), "Capture GetBufferSize");

        IntPtr pRenderService, pCaptureService;
        CableWasapi.ThrowIfFailed(renderClient.GetService( ref CableWasapi.IID_IAudioRenderClient,  out pRenderService),  "Render GetService");
        CableWasapi.ThrowIfFailed(captureClient.GetService(ref CableWasapi.IID_IAudioCaptureClient, out pCaptureService), "Capture GetService");
        var renderSvc  = (IAudioRenderClient) Marshal.GetObjectForIUnknown(pRenderService);
        var captureSvc = (IAudioCaptureClient)Marshal.GetObjectForIUnknown(pCaptureService);

        CableWasapi.ThrowIfFailed(captureClient.Start(), "Capture Start");
        CableWasapi.ThrowIfFailed(renderClient.Start(),  "Render Start");

        long capturedAbs = 0;
        try
        {
            for (int i = 0; i < 12; i++)
            {
                FillRenderBuffer(renderSvc, Math.Min(renderBufferFrames / 2, 480u), renderFormat);
                Thread.Sleep(35);
                capturedAbs += CaptureSignalBytes(captureSvc, captureFormat);
            }
        }
        finally
        {
            renderClient.Stop();
            captureClient.Stop();
            CableWasapi.CoTaskMemFree(pRenderFormat);
            CableWasapi.CoTaskMemFree(pCaptureFormat);
        }

        return "LOOPBACK PROBE: CapturedAbs=" + capturedAbs +
               " RenderFrames=" + renderBufferFrames +
               " CaptureFrames=" + captureBufferFrames;
    }
}
'@
}

Describe "Audio hardening: loopback virtual endpoint signal path" {
    BeforeAll {
        $script:Session = Reset-Vm @VmContext
        Copy-GuestAppExe -Session $script:Session -ExePath $VmContext.AppExePath -ReuseVm $VmContext.ReuseVm
        Start-GuestHeadlessApp -Session $script:Session
        Invoke-AppRpc -Session $script:Session -Method POST -Path "/driver/connect" | Out-Null
    }

    AfterAll {
        Stop-GuestHeadlessApp -Session $script:Session
        if ($script:Session) {
            Assert-NoGuestBugCheck -ComputerName $VmContext.ComputerName -Port $VmContext.Port -Username $VmContext.Username -Password $VmContext.Password -Context "Audio loopback hardening"
            Remove-PSSession $script:Session -ErrorAction SilentlyContinue
        }
    }

    It "plays render data through Cable runtime and observes non-zero capture" {
        $renderDevice  = $null
        $captureDevice = $null
        try {
            $renderDevice = Invoke-AppRpc -Session $script:Session -Method POST -Path "/virtual-devices" `
                -Body @{ name = "LoopbackTestSpeaker"; deviceType = "render" }
            $captureDevice = Invoke-AppRpc -Session $script:Session -Method POST -Path "/virtual-devices" `
                -Body @{ name = "LoopbackTestMic"; deviceType = "capture" }

            $renderDevice.endpointId  | Should -Not -BeNullOrEmpty
            $captureDevice.endpointId | Should -Not -BeNullOrEmpty

            # Build a routing graph: VirtualAudioOutput(render) → VirtualAudioInput(capture)
            $graph = @{
                nodes = @(
                    @{ type = "virtualAudioOutput"; data = @{ id = "n-render";  deviceId = $renderDevice.id;  name = $renderDevice.name  } },
                    @{ type = "virtualAudioInput";  data = @{ id = "n-capture"; deviceId = $captureDevice.id; name = $captureDevice.name } }
                )
                edges = @(
                    @{ id = "e1"; from = "n-render"; to = "n-capture" }
                )
            }
            Invoke-AppRpc -Session $script:Session -Method POST -Path "/graph" -Body $graph | Out-Null
            Invoke-AppRpc -Session $script:Session -Method POST -Path "/runtime/enable" | Out-Null
            Start-Sleep -Seconds 1

            $renderEndpointId  = $renderDevice.endpointId
            $captureEndpointId = $captureDevice.endpointId

            $result = Invoke-GuestCSharpTest -Session $script:Session `
                -CSharpSources @($script:wasapiCs, $script:loopbackCs) `
                -Script ([scriptblock]::Create(
                    "[CableAudioLoopbackProbe]::Run('$renderEndpointId', '$captureEndpointId')"
                )) `
                -TempFileName "audio-loopback-probe"

            $result | Should -Match '^LOOPBACK PROBE: CapturedAbs='
            $capturedAbs = [long]($result -replace '^.*CapturedAbs=([0-9]+).*$', '$1')
            $capturedAbs | Should -BeGreaterThan 0
        }
        finally {
            # Cleanup: stop runtime → empty graph → remove devices
            try { Invoke-AppRpc -Session $script:Session -Method POST -Path "/runtime/disable" | Out-Null } catch { Write-Host "Cleanup warning (disable runtime): $_" }
            try { Invoke-AppRpc -Session $script:Session -Method POST -Path "/graph" -Body @{ nodes = @(); edges = @() } | Out-Null } catch { Write-Host "Cleanup warning (empty graph): $_" }
            if ($captureDevice) {
                try { Invoke-AppRpc -Session $script:Session -Method DELETE -Path "/virtual-devices/$($captureDevice.id)" | Out-Null } catch { Write-Host "Cleanup warning (delete capture): $_" }
            }
            if ($renderDevice) {
                try { Invoke-AppRpc -Session $script:Session -Method DELETE -Path "/virtual-devices/$($renderDevice.id)" | Out-Null } catch { Write-Host "Cleanup warning (delete render): $_" }
            }
        }
    }
}
