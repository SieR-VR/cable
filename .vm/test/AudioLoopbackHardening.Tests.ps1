# Audio loopback regression test for virtual Cable endpoints.
# Goal: verify that audio written to render endpoint is captured by virtual microphone path.
# The driver creates endpoints dynamically via IOCTL, so this test creates
# a speaker + mic pair before running the WASAPI loopback probe.

BeforeAll {
    . (Join-Path $PSScriptRoot "common.ps1")
    $script:ioctlCs  = Get-CSharpLib "CableIoctl"
    $script:wasapiCs  = Get-CSharpLib "CableWasapi"

    # Test-specific C# class: creates virtual devices via IOCTL, waits for
    # WASAPI endpoints, runs render→capture loopback, then cleans up.
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
            uint frames;
            uint flags;
            ulong pos, qpc;
            CableWasapi.ThrowIfFailed(captureClient.GetBuffer(out pData, out frames, out flags, out pos, out qpc), "GetBuffer");

            int bytes = checked((int)(frames * format.nBlockAlign));
            if (bytes > 0)
            {
                byte[] buf = new byte[bytes];
                Marshal.Copy(pData, buf, 0, bytes);
                for (int i = 0; i < buf.Length; i++)
                {
                    totalAbs += Math.Abs((int)buf[i] - 128);
                }
            }

            CableWasapi.ThrowIfFailed(captureClient.ReleaseBuffer(frames), "ReleaseBuffer");
            CableWasapi.ThrowIfFailed(captureClient.GetNextPacketSize(out nextPacket), "GetNextPacketSize(loop)");
        }

        return totalAbs;
    }

    public static string Run(string endpointNameContains)
    {
        // Step 1: Create virtual speaker (render=0) and mic (capture=1) via IOCTL
        string createRender = CableIoctl.Create(0, "LoopbackTestSpeaker");
        if (!createRender.StartsWith("CREATE OK"))
            return "SETUP_FAIL: render create: " + createRender;
        string renderId = CableIoctl.ParseCreateId(createRender);

        string createCapture = CableIoctl.Create(1, "LoopbackTestMic");
        if (!createCapture.StartsWith("CREATE OK"))
        {
            CableIoctl.Remove(renderId);
            return "SETUP_FAIL: capture create: " + createCapture;
        }
        string captureId = CableIoctl.ParseCreateId(createCapture);

        try
        {
            // Step 2: Wait for WASAPI endpoints to appear
            IMMDevice renderDev = null;
            IMMDevice captureDev = null;
            for (int attempt = 0; attempt < 40; attempt++)
            {
                Thread.Sleep(500);
                if (renderDev == null)
                    renderDev = CableWasapi.FindDeviceByName(0, endpointNameContains);
                if (captureDev == null)
                    captureDev = CableWasapi.FindDeviceByName(1, endpointNameContains);
                if (renderDev != null && captureDev != null) break;
            }

            if (renderDev == null || captureDev == null)
            {
                string msg = "ENDPOINT_NOT_FOUND: render=" + (renderDev != null) + " capture=" + (captureDev != null);
                msg += "\nAll render: " + CableWasapi.ListAllDevices(0);
                msg += "\nAll capture: " + CableWasapi.ListAllDevices(1);
                return msg;
            }

            // Step 3: Run loopback probe
            var renderClient = CableWasapi.ActivateAudioClient(renderDev);
            var captureClient = CableWasapi.ActivateAudioClient(captureDev);

            IntPtr pRenderFormat = CableWasapi.GetMixFormatPtr(renderClient);
            IntPtr pCaptureFormat = CableWasapi.GetMixFormatPtr(captureClient);
            WAVEFORMATEX renderFormat = (WAVEFORMATEX)Marshal.PtrToStructure(pRenderFormat, typeof(WAVEFORMATEX));
            WAVEFORMATEX captureFormat = (WAVEFORMATEX)Marshal.PtrToStructure(pCaptureFormat, typeof(WAVEFORMATEX));

            long hnsBuffer = 2000000;
            CableWasapi.ThrowIfFailed(renderClient.Initialize(CableWasapi.AUDCLNT_SHAREMODE_SHARED, 0, hnsBuffer, 0, pRenderFormat, IntPtr.Zero), "Render Initialize");
            CableWasapi.ThrowIfFailed(captureClient.Initialize(CableWasapi.AUDCLNT_SHAREMODE_SHARED, 0, hnsBuffer, 0, pCaptureFormat, IntPtr.Zero), "Capture Initialize");

            uint renderBufferFrames;
            CableWasapi.ThrowIfFailed(renderClient.GetBufferSize(out renderBufferFrames), "Render GetBufferSize");

            uint captureBufferFrames;
            CableWasapi.ThrowIfFailed(captureClient.GetBufferSize(out captureBufferFrames), "Capture GetBufferSize");

            IntPtr pRenderService;
            CableWasapi.ThrowIfFailed(renderClient.GetService(ref CableWasapi.IID_IAudioRenderClient, out pRenderService), "Render GetService");
            var renderSvc = (IAudioRenderClient)Marshal.GetObjectForIUnknown(pRenderService);

            IntPtr pCaptureService;
            CableWasapi.ThrowIfFailed(captureClient.GetService(ref CableWasapi.IID_IAudioCaptureClient, out pCaptureService), "Capture GetService");
            var captureSvc = (IAudioCaptureClient)Marshal.GetObjectForIUnknown(pCaptureService);

            CableWasapi.ThrowIfFailed(captureClient.Start(), "Capture Start");
            CableWasapi.ThrowIfFailed(renderClient.Start(), "Render Start");

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

            return "LOOPBACK PROBE: CapturedAbs=" + capturedAbs + ", RenderFrames=" + renderBufferFrames + ", CaptureFrames=" + captureBufferFrames;
        }
        finally
        {
            // Step 4: Cleanup virtual devices
            CableIoctl.RemoveWithRetry(captureId, 5, 500);
            CableIoctl.RemoveWithRetry(renderId, 5, 500);
        }
    }
}
'@
}

Describe "Audio hardening: loopback virtual endpoint signal path" {
    BeforeAll {
        $script:Session = Reset-Vm @VmContext
    }

    AfterAll {
        if ($script:Session) {
            Assert-NoGuestBugCheck -ComputerName $VmContext.ComputerName -Port $VmContext.Port -Username $VmContext.Username -Password $VmContext.Password -Context "Audio loopback hardening"
            Remove-PSSession $script:Session -ErrorAction SilentlyContinue
        }
    }

    It "plays render data and observes non-zero capture activity" {
        $result = Invoke-GuestCSharpTest -Session $script:Session `
            -CSharpSources @($script:ioctlCs, $script:wasapiCs, $script:loopbackCs) `
            -Script {
                [CableAudioLoopbackProbe]::Run("Cable Virtual Audio")
            } `
            -TempFileName "audio-loopback-probe"

        $result | Should -Match '^LOOPBACK PROBE: CapturedAbs='

        $capturedAbs = [long](($result -replace '^.*CapturedAbs=([0-9]+).*$','$1'))
        $capturedAbs | Should -BeGreaterThan 0
    }
}
