# E2E VM test: render audio to Cable virtual speaker and verify it appears
# in the loopback capture stream of the same endpoint.
# The driver creates endpoints dynamically via IOCTL, so we create a
# virtual speaker first, wait for it, then render + loopback-capture.

BeforeAll {
    . (Join-Path $PSScriptRoot "common.ps1")
    $script:ioctlCs  = Get-CSharpLib "CableIoctl"
    $script:wasapiCs  = Get-CSharpLib "CableWasapi"

    $script:e2eCs = @'
public static class WasapiLoopbackE2E
{
    static void RenderPulse(IAudioRenderClient rc, uint frames, WAVEFORMATEX f)
    {
        IntPtr p;
        CableWasapi.ThrowIfFailed(rc.GetBuffer(frames, out p), "Render GetBuffer");
        int bytes = checked((int)(frames * f.nBlockAlign));
        byte[] b = new byte[bytes];

        if (f.wBitsPerSample == 16)
        {
            int sampleCount = bytes / 2;
            for (int i = 0; i < sampleCount; i++)
            {
                short s = (short)((i % 64 < 32) ? 12000 : -12000);
                byte[] bs = BitConverter.GetBytes(s);
                b[i * 2] = bs[0];
                b[i * 2 + 1] = bs[1];
            }
        }
        else
        {
            int sampleCount = bytes / 4;
            for (int i = 0; i < sampleCount; i++)
            {
                float s = (i % 64 < 32) ? 0.4f : -0.4f;
                byte[] bs = BitConverter.GetBytes(s);
                Buffer.BlockCopy(bs, 0, b, i * 4, 4);
            }
        }

        Marshal.Copy(b, 0, p, bytes);
        CableWasapi.ThrowIfFailed(rc.ReleaseBuffer(frames, 0), "Render ReleaseBuffer");
    }

    static long CaptureAbs(IAudioCaptureClient cc, WAVEFORMATEX f)
    {
        long acc = 0;
        uint next;
        CableWasapi.ThrowIfFailed(cc.GetNextPacketSize(out next), "Capture GetNextPacketSize");
        while (next > 0)
        {
            IntPtr p;
            uint frames;
            uint flags;
            ulong pos, qpc;
            CableWasapi.ThrowIfFailed(cc.GetBuffer(out p, out frames, out flags, out pos, out qpc), "Capture GetBuffer");
            int bytes = checked((int)(frames * f.nBlockAlign));
            if (bytes > 0)
            {
                byte[] buf = new byte[bytes];
                Marshal.Copy(p, buf, 0, bytes);
                foreach (var v in buf) { acc += Math.Abs((int)v - 128); }
            }
            CableWasapi.ThrowIfFailed(cc.ReleaseBuffer(frames), "Capture ReleaseBuffer");
            CableWasapi.ThrowIfFailed(cc.GetNextPacketSize(out next), "Capture GetNextPacketSize(loop)");
        }
        return acc;
    }

    public static string Run(string endpointNameContains)
    {
        // Create a virtual speaker via IOCTL
        string createResult = CableIoctl.Create(0, "E2eTestSpeaker");
        if (!createResult.StartsWith("CREATE OK"))
            return "SETUP_FAIL: " + createResult;
        string deviceId = CableIoctl.ParseCreateId(createResult);

        try
        {
            // Wait for WASAPI render endpoint
            IMMDevice speaker = null;
            for (int attempt = 0; attempt < 40; attempt++)
            {
                Thread.Sleep(500);
                speaker = CableWasapi.FindDeviceByName(0, endpointNameContains);
                if (speaker != null) break;
            }
            if (speaker == null)
                return "ENDPOINT_NOT_FOUND: " + CableWasapi.ListAllDevices(0);

            // Open for render + loopback capture on the SAME endpoint
            var renderClient = CableWasapi.ActivateAudioClient(speaker);
            var loopClient   = CableWasapi.ActivateAudioClient(speaker);

            IntPtr pRenderFmt = CableWasapi.GetMixFormatPtr(renderClient);
            IntPtr pLoopFmt   = CableWasapi.GetMixFormatPtr(loopClient);

            WAVEFORMATEX renderFmt = (WAVEFORMATEX)Marshal.PtrToStructure(pRenderFmt, typeof(WAVEFORMATEX));
            WAVEFORMATEX loopFmt   = (WAVEFORMATEX)Marshal.PtrToStructure(pLoopFmt, typeof(WAVEFORMATEX));

            try
            {
                long hns = 2000000;
                CableWasapi.ThrowIfFailed(renderClient.Initialize(
                    CableWasapi.AUDCLNT_SHAREMODE_SHARED, 0, hns, 0, pRenderFmt, IntPtr.Zero),
                    "Render Initialize");
                CableWasapi.ThrowIfFailed(loopClient.Initialize(
                    CableWasapi.AUDCLNT_SHAREMODE_SHARED, CableWasapi.AUDCLNT_STREAMFLAGS_LOOPBACK,
                    hns, 0, pLoopFmt, IntPtr.Zero),
                    "Loop Initialize");

                uint rb;
                CableWasapi.ThrowIfFailed(renderClient.GetBufferSize(out rb), "Render GetBufferSize");

                IntPtr pR;
                CableWasapi.ThrowIfFailed(renderClient.GetService(ref CableWasapi.IID_IAudioRenderClient, out pR), "Render GetService");
                var rsvc = (IAudioRenderClient)Marshal.GetObjectForIUnknown(pR);

                IntPtr pC;
                CableWasapi.ThrowIfFailed(loopClient.GetService(ref CableWasapi.IID_IAudioCaptureClient, out pC), "Loop GetService");
                var csvc = (IAudioCaptureClient)Marshal.GetObjectForIUnknown(pC);

                CableWasapi.ThrowIfFailed(loopClient.Start(), "Loop Start");
                CableWasapi.ThrowIfFailed(renderClient.Start(), "Render Start");

                long abs = 0;
                for (int i = 0; i < 15; i++)
                {
                    RenderPulse(rsvc, Math.Min(rb / 2, 480u), renderFmt);
                    Thread.Sleep(40);
                    abs += CaptureAbs(csvc, loopFmt);
                }

                renderClient.Stop();
                loopClient.Stop();

                return "E2E LOOPBACK: abs=" + abs + ", rb=" + rb;
            }
            finally
            {
                CableWasapi.CoTaskMemFree(pRenderFmt);
                CableWasapi.CoTaskMemFree(pLoopFmt);
            }
        }
        finally
        {
            CableIoctl.RemoveWithRetry(deviceId, 5, 500);
        }
    }
}
'@
}

Describe "Audio hardening: e2e loopback render path" {
    BeforeAll {
        $script:Session = Reset-Vm @VmContext
    }

    AfterAll {
        if ($script:Session) {
            Assert-NoGuestBugCheck -ComputerName $VmContext.ComputerName -Port $VmContext.Port -Username $VmContext.Username -Password $VmContext.Password -Context "Audio e2e loopback"
            Remove-PSSession $script:Session -ErrorAction SilentlyContinue
        }
    }

    It "renders audio to virtual speaker and captures via loopback" {
        $result = Invoke-GuestCSharpTest -Session $script:Session `
            -CSharpSources @($script:ioctlCs, $script:wasapiCs, $script:e2eCs) `
            -Script {
                [WasapiLoopbackE2E]::Run("Cable Virtual Audio")
            } `
            -TempFileName "e2e-loopback-flow"

        $result | Should -Match '^E2E LOOPBACK: abs='
        $abs = [long](($result -replace '^.*abs=([0-9]+).*$','$1'))
        $abs | Should -BeGreaterThan 0
    }
}
