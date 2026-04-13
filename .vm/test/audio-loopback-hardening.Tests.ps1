# Audio loopback regression test for virtual Cable endpoints.
# Goal: verify that audio written to render endpoint is captured by virtual microphone path.

BeforeAll {
    . (Join-Path $PSScriptRoot "common.ps1")

    function script:Invoke-AudioLoopbackProbe {
        param(
            [System.Management.Automation.Runspaces.PSSession]$Session,
            [string]$TargetContains = "Cable Virtual Audio Device"
        )

        $probeCs = @'
using System;
using System.Runtime.InteropServices;
using System.Threading;

public static class CableAudioLoopbackProbe
{
    [StructLayout(LayoutKind.Sequential)]
    public struct WAVEFORMATEX
    {
        public ushort wFormatTag;
        public ushort nChannels;
        public uint nSamplesPerSec;
        public uint nAvgBytesPerSec;
        public ushort nBlockAlign;
        public ushort wBitsPerSample;
        public ushort cbSize;
    }

    [ComImport, Guid("A95664D2-9614-4F35-A746-DE8DB63617E6"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    public interface IMMDeviceEnumerator
    {
        int EnumAudioEndpoints(int dataFlow, uint dwStateMask, out IMMDeviceCollection devices);
        int GetDefaultAudioEndpoint(int dataFlow, int role, out IMMDevice device);
        int GetDevice([MarshalAs(UnmanagedType.LPWStr)] string id, out IMMDevice device);
        int RegisterEndpointNotificationCallback(IntPtr pClient);
        int UnregisterEndpointNotificationCallback(IntPtr pClient);
    }

    [ComImport, Guid("0BD7A1BE-7A1A-44DB-8397-CC5392387B5E"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    public interface IMMDeviceCollection
    {
        int GetCount(out uint count);
        int Item(uint index, out IMMDevice device);
    }

    [ComImport, Guid("D666063F-1587-4E43-81F1-B948E807363F"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    public interface IMMDevice
    {
        int Activate(ref Guid iid, uint clsCtx, IntPtr activationParams, out IntPtr interfacePtr);
        int OpenPropertyStore(int stgmAccess, out IPropertyStore properties);
        int GetId([MarshalAs(UnmanagedType.LPWStr)] out string id);
        int GetState(out uint state);
    }

    [ComImport, Guid("886d8eeb-8cf2-4446-8d02-cdba1dbdcf99"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    public interface IPropertyStore
    {
        int GetCount(out uint cProps);
        int GetAt(uint iProp, out PROPERTYKEY pkey);
        int GetValue(ref PROPERTYKEY key, out PROPVARIANT pv);
        int SetValue(ref PROPERTYKEY key, ref PROPVARIANT pv);
        int Commit();
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct PROPERTYKEY
    {
        public Guid fmtid;
        public uint pid;
    }

    [StructLayout(LayoutKind.Explicit)]
    public struct PROPVARIANT
    {
        [FieldOffset(0)] public ushort vt;
        [FieldOffset(8)] public IntPtr pointerValue;
    }

    [ComImport, Guid("1CB9AD4C-DBFA-4c32-B178-C2F568A703B2"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    public interface IAudioClient
    {
        int Initialize(uint shareMode, uint streamFlags, long hnsBufferDuration, long hnsPeriodicity, IntPtr format, IntPtr audioSessionGuid);
        int GetBufferSize(out uint bufferSize);
        int GetStreamLatency(out long latency);
        int GetCurrentPadding(out uint currentPadding);
        int IsFormatSupported(uint shareMode, ref WAVEFORMATEX format, IntPtr closestMatch);
        int GetMixFormat(out IntPtr deviceFormat);
        int GetDevicePeriod(out long defaultDevicePeriod, out long minimumDevicePeriod);
        int Start();
        int Stop();
        int Reset();
        int SetEventHandle(IntPtr eventHandle);
        int GetService(ref Guid iid, out IntPtr service);
    }

    [ComImport, Guid("F294ACFC-3146-4483-A7BF-ADDCA7C260E2"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    public interface IAudioRenderClient
    {
        int GetBuffer(uint numFramesRequested, out IntPtr dataBufferPointer);
        int ReleaseBuffer(uint numFramesWritten, uint flags);
    }

    [ComImport, Guid("C8ADBD64-E71E-48a0-A4DE-185C395CD317"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    public interface IAudioCaptureClient
    {
        int GetBuffer(out IntPtr data, out uint numFramesToRead, out uint flags, out ulong devicePosition, out ulong qpcPosition);
        int ReleaseBuffer(uint numFramesRead);
        int GetNextPacketSize(out uint numFramesInNextPacket);
    }

    [DllImport("ole32.dll")]
    private static extern int PropVariantClear(ref PROPVARIANT pvar);

    [DllImport("ole32.dll")]
    private static extern void CoTaskMemFree(IntPtr ptr);

    static Guid CLSID_MMDeviceEnumerator = new Guid("BCDE0395-E52F-467C-8E3D-C4579291692E");
    static Guid IID_IAudioClient = new Guid("1CB9AD4C-DBFA-4c32-B178-C2F568A703B2");
    static Guid IID_IAudioRenderClient = new Guid("F294ACFC-3146-4483-A7BF-ADDCA7C260E2");
    static Guid IID_IAudioCaptureClient = new Guid("C8ADBD64-E71E-48a0-A4DE-185C395CD317");

    const uint AUDCLNT_SHAREMODE_SHARED = 0;
    const uint AUDCLNT_STREAMFLAGS_EVENTCALLBACK = 0x00040000;
    const int STGM_READ = 0;

    static void ThrowIfFailed(int hr, string op)
    {
        if (hr < 0) Marshal.ThrowExceptionForHR(hr, new IntPtr(-1));
    }

    static string ReadFriendlyName(IMMDevice device)
    {
        IPropertyStore store;
        ThrowIfFailed(device.OpenPropertyStore(STGM_READ, out store), "OpenPropertyStore");

        PROPERTYKEY key = new PROPERTYKEY
        {
            fmtid = new Guid("A45C254E-DF1C-4EFD-8020-67D146A850E0"),
            pid = 14
        };

        PROPVARIANT pv;
        ThrowIfFailed(store.GetValue(ref key, out pv), "GetValue(FriendlyName)");

        try
        {
            if (pv.vt == 31 && pv.pointerValue != IntPtr.Zero)
            {
                return Marshal.PtrToStringUni(pv.pointerValue) ?? string.Empty;
            }
            return string.Empty;
        }
        finally
        {
            PropVariantClear(ref pv);
        }
    }

    static IMMDevice FindDeviceByName(int dataFlow, string contains)
    {
        var enumerator = (IMMDeviceEnumerator)Activator.CreateInstance(Type.GetTypeFromCLSID(CLSID_MMDeviceEnumerator));
        IMMDeviceCollection collection;
        ThrowIfFailed(enumerator.EnumAudioEndpoints(dataFlow, 1, out collection), "EnumAudioEndpoints");

        uint count;
        ThrowIfFailed(collection.GetCount(out count), "GetCount");

        for (uint i = 0; i < count; i++)
        {
            IMMDevice device;
            ThrowIfFailed(collection.Item(i, out device), "Item");

            string name = ReadFriendlyName(device);
            if (!string.IsNullOrEmpty(name) &&
                name.IndexOf(contains, StringComparison.OrdinalIgnoreCase) >= 0)
            {
                return device;
            }
        }

        throw new InvalidOperationException("Target device not found: " + contains + " (dataFlow=" + dataFlow + ")");
    }

    static IAudioClient ActivateAudioClient(IMMDevice device)
    {
        IntPtr ptr;
        ThrowIfFailed(device.Activate(ref IID_IAudioClient, 23, IntPtr.Zero, out ptr), "IMMDevice::Activate(IAudioClient)");
        return (IAudioClient)Marshal.GetObjectForIUnknown(ptr);
    }

    static IntPtr GetMixFormatPtr(IAudioClient client)
    {
        IntPtr pFormat;
        ThrowIfFailed(client.GetMixFormat(out pFormat), "IAudioClient::GetMixFormat");
        return pFormat;
    }

    static void FillRenderBuffer(IAudioRenderClient renderClient, uint frames, WAVEFORMATEX format)
    {
        IntPtr pData;
        ThrowIfFailed(renderClient.GetBuffer(frames, out pData), "IAudioRenderClient::GetBuffer");
        int bytes = checked((int)(frames * format.nBlockAlign));

        // 1kHz square wave (simple high-amplitude pattern)
        byte[] block = new byte[bytes];
        int frameStride = Math.Max(1, (int)format.nBlockAlign);
        for (int i = 0; i < bytes; i++)
        {
            block[i] = (byte)((i / frameStride) % 2 == 0 ? 0x10 : 0xF0);
        }

        Marshal.Copy(block, 0, pData, bytes);
        ThrowIfFailed(renderClient.ReleaseBuffer(frames, 0), "IAudioRenderClient::ReleaseBuffer");
    }

    static long CaptureSignalBytes(IAudioCaptureClient captureClient, WAVEFORMATEX format)
    {
        long totalAbs = 0;
        uint nextPacket;

        ThrowIfFailed(captureClient.GetNextPacketSize(out nextPacket), "IAudioCaptureClient::GetNextPacketSize");
        while (nextPacket > 0)
        {
            IntPtr pData;
            uint frames;
            uint flags;
            ulong pos, qpc;
            ThrowIfFailed(captureClient.GetBuffer(out pData, out frames, out flags, out pos, out qpc), "IAudioCaptureClient::GetBuffer");

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

            ThrowIfFailed(captureClient.ReleaseBuffer(frames), "IAudioCaptureClient::ReleaseBuffer");
            ThrowIfFailed(captureClient.GetNextPacketSize(out nextPacket), "IAudioCaptureClient::GetNextPacketSize(loop)");
        }

        return totalAbs;
    }

    public static string Run(string targetContains)
    {
        IMMDevice render = FindDeviceByName(0, targetContains);
        IMMDevice capture = FindDeviceByName(1, targetContains);

        var renderClient = ActivateAudioClient(render);
        var captureClient = ActivateAudioClient(capture);

        IntPtr pRenderFormat = GetMixFormatPtr(renderClient);
        IntPtr pCaptureFormat = GetMixFormatPtr(captureClient);
        WAVEFORMATEX renderFormat = (WAVEFORMATEX)Marshal.PtrToStructure(pRenderFormat, typeof(WAVEFORMATEX));
        WAVEFORMATEX captureFormat = (WAVEFORMATEX)Marshal.PtrToStructure(pCaptureFormat, typeof(WAVEFORMATEX));

        long hnsBuffer = 2000000; // 200ms
        ThrowIfFailed(renderClient.Initialize(AUDCLNT_SHAREMODE_SHARED, 0, hnsBuffer, 0, pRenderFormat, IntPtr.Zero), "Render Initialize");
        ThrowIfFailed(captureClient.Initialize(AUDCLNT_SHAREMODE_SHARED, 0, hnsBuffer, 0, pCaptureFormat, IntPtr.Zero), "Capture Initialize");

        uint renderBufferFrames;
        ThrowIfFailed(renderClient.GetBufferSize(out renderBufferFrames), "Render GetBufferSize");

        uint captureBufferFrames;
        ThrowIfFailed(captureClient.GetBufferSize(out captureBufferFrames), "Capture GetBufferSize");

        IntPtr pRenderService;
        ThrowIfFailed(renderClient.GetService(ref IID_IAudioRenderClient, out pRenderService), "Render GetService");
        var renderSvc = (IAudioRenderClient)Marshal.GetObjectForIUnknown(pRenderService);

        IntPtr pCaptureService;
        ThrowIfFailed(captureClient.GetService(ref IID_IAudioCaptureClient, out pCaptureService), "Capture GetService");
        var captureSvc = (IAudioCaptureClient)Marshal.GetObjectForIUnknown(pCaptureService);

        ThrowIfFailed(captureClient.Start(), "Capture Start");
        ThrowIfFailed(renderClient.Start(), "Render Start");

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
            if (pRenderFormat != IntPtr.Zero) { CoTaskMemFree(pRenderFormat); }
            if (pCaptureFormat != IntPtr.Zero) { CoTaskMemFree(pCaptureFormat); }
        }

        return "LOOPBACK PROBE: CapturedAbs=" + capturedAbs + ", RenderFrames=" + renderBufferFrames + ", CaptureFrames=" + captureBufferFrames;
    }
}
'@

        $scriptText = @"
`$ErrorActionPreference = 'Stop'
Add-Type -TypeDefinition @'
$probeCs
'@
[CableAudioLoopbackProbe]::Run('$TargetContains')
"@

        return Invoke-Command -Session $Session -ScriptBlock {
            param($payload)
            $tmp = "C:\CableAudio\audio-loopback-probe.ps1"
            Set-Content -Path $tmp -Value $payload -Encoding UTF8
            & powershell -NoProfile -ExecutionPolicy Bypass -File $tmp
        } -ArgumentList $scriptText
    }
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
        $result = Invoke-AudioLoopbackProbe -Session $script:Session
        $result | Should -Match '^LOOPBACK PROBE: CapturedAbs='

        $capturedAbs = [long](($result -replace '^.*CapturedAbs=([0-9]+).*$','$1'))
        $capturedAbs | Should -BeGreaterThan 0
    }
}
