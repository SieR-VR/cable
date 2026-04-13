# E2E VM test for virtual speaker -> real output flow using local WAV playback.

BeforeAll {
    . (Join-Path $PSScriptRoot "common.ps1")
}

Describe "Audio hardening: e2e file playback flow" {
    BeforeAll {
        $script:Session = Reset-Vm @VmContext
    }

    AfterAll {
        if ($script:Session) {
            Assert-NoGuestBugCheck -ComputerName $VmContext.ComputerName -Port $VmContext.Port -Username $VmContext.Username -Password $VmContext.Password -Context "Audio e2e file playback flow"
            Remove-PSSession $script:Session -ErrorAction SilentlyContinue
        }
    }

    It "routes generated audio from virtual speaker to output chain" {
        $scriptBlock = @'
$ErrorActionPreference = "Stop"

Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
using System.Threading;

public static class WasapiLoopbackE2E
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
        int IsFormatSupported(uint shareMode, IntPtr format, IntPtr closestMatch);
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
    const uint AUDCLNT_STREAMFLAGS_LOOPBACK = 0x00020000;

    static void ThrowIfFailed(int hr, string op)
    {
        if (hr < 0) Marshal.ThrowExceptionForHR(hr, new IntPtr(-1));
    }

    static string ReadFriendlyName(IMMDevice device)
    {
        IPropertyStore store;
        ThrowIfFailed(device.OpenPropertyStore(0, out store), "OpenPropertyStore");

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

    static IMMDevice FindDeviceByName(int flow, string contains)
    {
        var enumerator = (IMMDeviceEnumerator)Activator.CreateInstance(Type.GetTypeFromCLSID(CLSID_MMDeviceEnumerator));
        IMMDeviceCollection collection;
        ThrowIfFailed(enumerator.EnumAudioEndpoints(flow, 1, out collection), "EnumAudioEndpoints");

        uint count;
        ThrowIfFailed(collection.GetCount(out count), "GetCount");

        for (uint i = 0; i < count; i++)
        {
            IMMDevice d;
            ThrowIfFailed(collection.Item(i, out d), "Item");
            var name = ReadFriendlyName(d);
            if (name.IndexOf(contains, StringComparison.OrdinalIgnoreCase) >= 0)
            {
                return d;
            }
        }

        throw new InvalidOperationException("Device not found: " + contains);
    }

    static IAudioClient ActivateAudioClient(IMMDevice d)
    {
        IntPtr p;
        ThrowIfFailed(d.Activate(ref IID_IAudioClient, 23, IntPtr.Zero, out p), "Activate IAudioClient");
        return (IAudioClient)Marshal.GetObjectForIUnknown(p);
    }

    static void RenderPulse(IAudioRenderClient rc, uint frames, WAVEFORMATEX f)
    {
        IntPtr p;
        ThrowIfFailed(rc.GetBuffer(frames, out p), "Render GetBuffer");
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
        ThrowIfFailed(rc.ReleaseBuffer(frames, 0), "Render ReleaseBuffer");
    }

    static long CaptureAbs(IAudioCaptureClient cc, WAVEFORMATEX f)
    {
        long acc = 0;
        uint next;
        ThrowIfFailed(cc.GetNextPacketSize(out next), "Capture GetNextPacketSize");
        while (next > 0)
        {
            IntPtr p;
            uint frames;
            uint flags;
            ulong pos, qpc;
            ThrowIfFailed(cc.GetBuffer(out p, out frames, out flags, out pos, out qpc), "Capture GetBuffer");
            int bytes = checked((int)(frames * f.nBlockAlign));
            if (bytes > 0)
            {
                byte[] b = new byte[bytes];
                Marshal.Copy(p, b, 0, bytes);
                foreach (var v in b) { acc += Math.Abs((int)v - 128); }
            }
            ThrowIfFailed(cc.ReleaseBuffer(frames), "Capture ReleaseBuffer");
            ThrowIfFailed(cc.GetNextPacketSize(out next), "Capture GetNextPacketSize(loop)");
        }
        return acc;
    }

    public static string Run(string virtualSpeakerNameContains, string realOutputContains)
    {
        IMMDevice virtualSpeaker = FindDeviceByName(0, virtualSpeakerNameContains);
        IMMDevice realOutput = FindDeviceByName(0, realOutputContains);

        var renderClient = ActivateAudioClient(virtualSpeaker);
        var loopClient = ActivateAudioClient(realOutput);

        IntPtr pRenderFmt;
        ThrowIfFailed(renderClient.GetMixFormat(out pRenderFmt), "Render GetMixFormat");
        IntPtr pLoopFmt;
        ThrowIfFailed(loopClient.GetMixFormat(out pLoopFmt), "Loop GetMixFormat");

        WAVEFORMATEX renderFmt = (WAVEFORMATEX)Marshal.PtrToStructure(pRenderFmt, typeof(WAVEFORMATEX));
        WAVEFORMATEX loopFmt = (WAVEFORMATEX)Marshal.PtrToStructure(pLoopFmt, typeof(WAVEFORMATEX));

        try
        {
            long hns = 2000000;
            ThrowIfFailed(renderClient.Initialize(AUDCLNT_SHAREMODE_SHARED, 0, hns, 0, pRenderFmt, IntPtr.Zero), "Render Initialize");
            ThrowIfFailed(loopClient.Initialize(AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK, hns, 0, pLoopFmt, IntPtr.Zero), "Loop Initialize");

            uint rb; ThrowIfFailed(renderClient.GetBufferSize(out rb), "Render GetBufferSize");

            IntPtr pR; ThrowIfFailed(renderClient.GetService(ref IID_IAudioRenderClient, out pR), "Render GetService");
            var rsvc = (IAudioRenderClient)Marshal.GetObjectForIUnknown(pR);

            IntPtr pC; ThrowIfFailed(loopClient.GetService(ref IID_IAudioCaptureClient, out pC), "Loop GetService");
            var csvc = (IAudioCaptureClient)Marshal.GetObjectForIUnknown(pC);

            ThrowIfFailed(loopClient.Start(), "Loop Start");
            ThrowIfFailed(renderClient.Start(), "Render Start");

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
            CoTaskMemFree(pRenderFmt);
            CoTaskMemFree(pLoopFmt);
        }
    }
}
"@

[WasapiLoopbackE2E]::Run("Cable Virtual Audio Device", "Speakers")
'@

        $result = Invoke-Command -Session $script:Session -ScriptBlock {
            param($body)
            $tmp = "C:\CableAudio\e2e-loopback-flow.ps1"
            Set-Content -Path $tmp -Value $body -Encoding UTF8
            & powershell -NoProfile -ExecutionPolicy Bypass -File $tmp
        } -ArgumentList $scriptBlock

        $result | Should -Match '^E2E LOOPBACK: abs='
        $abs = [long](($result -replace '^.*abs=([0-9]+).*$','$1'))
        $abs | Should -BeGreaterThan 0
    }
}
