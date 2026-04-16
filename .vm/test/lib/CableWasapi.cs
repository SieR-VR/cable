// Shared WASAPI COM interop for CableAudio driver VM tests.
// Provides audio endpoint enumeration, property access, and streaming helpers.
//
// Used by: RingBufferDataflow.Tests.ps1, AudioLoopbackHardening.Tests.ps1,
//          E2eMp3FlowHardening.Tests.ps1, PkeyFriendlyName.Tests.ps1

using System;
using System.Runtime.InteropServices;
using System.Threading;

// --- Shared structs ---

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

// --- COM interfaces (vtable order must match Windows SDK) ---

[ComImport, Guid("A95664D2-9614-4F35-A746-DE8DB63617E6"),
 InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
public interface IMMDeviceEnumerator
{
    [PreserveSig] int EnumAudioEndpoints(int dataFlow, uint dwStateMask, out IMMDeviceCollection devices);
    [PreserveSig] int GetDefaultAudioEndpoint(int dataFlow, int role, out IMMDevice device);
    [PreserveSig] int GetDevice([MarshalAs(UnmanagedType.LPWStr)] string id, out IMMDevice device);
    [PreserveSig] int RegisterEndpointNotificationCallback(IntPtr pClient);
    [PreserveSig] int UnregisterEndpointNotificationCallback(IntPtr pClient);
}

[ComImport, Guid("0BD7A1BE-7A1A-44DB-8397-CC5392387B5E"),
 InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
public interface IMMDeviceCollection
{
    [PreserveSig] int GetCount(out uint count);
    [PreserveSig] int Item(uint index, out IMMDevice device);
}

[ComImport, Guid("D666063F-1587-4E43-81F1-B948E807363F"),
 InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
public interface IMMDevice
{
    [PreserveSig] int Activate(ref Guid iid, uint clsCtx, IntPtr activationParams, out IntPtr interfacePtr);
    [PreserveSig] int OpenPropertyStore(int stgmAccess, out IPropertyStore properties);
    [PreserveSig] int GetId([MarshalAs(UnmanagedType.LPWStr)] out string id);
    [PreserveSig] int GetState(out uint state);
}

[ComImport, Guid("886d8eeb-8cf2-4446-8d02-cdba1dbdcf99"),
 InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
public interface IPropertyStore
{
    [PreserveSig] int GetCount(out uint cProps);
    [PreserveSig] int GetAt(uint iProp, out PROPERTYKEY pkey);
    [PreserveSig] int GetValue(ref PROPERTYKEY key, out PROPVARIANT pv);
    [PreserveSig] int SetValue(ref PROPERTYKEY key, ref PROPVARIANT pv);
    [PreserveSig] int Commit();
}

[ComImport, Guid("1CB9AD4C-DBFA-4c32-B178-C2F568A703B2"),
 InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
public interface IAudioClient
{
    [PreserveSig] int Initialize(uint shareMode, uint streamFlags, long hnsBufferDuration, long hnsPeriodicity, IntPtr format, IntPtr audioSessionGuid);
    [PreserveSig] int GetBufferSize(out uint bufferSize);
    [PreserveSig] int GetStreamLatency(out long latency);
    [PreserveSig] int GetCurrentPadding(out uint currentPadding);
    [PreserveSig] int IsFormatSupported(uint shareMode, IntPtr format, IntPtr closestMatch);
    [PreserveSig] int GetMixFormat(out IntPtr deviceFormat);
    [PreserveSig] int GetDevicePeriod(out long defaultDevicePeriod, out long minimumDevicePeriod);
    [PreserveSig] int Start();
    [PreserveSig] int Stop();
    [PreserveSig] int Reset();
    [PreserveSig] int SetEventHandle(IntPtr eventHandle);
    [PreserveSig] int GetService(ref Guid iid, out IntPtr service);
}

[ComImport, Guid("F294ACFC-3146-4483-A7BF-ADDCA7C260E2"),
 InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
public interface IAudioRenderClient
{
    [PreserveSig] int GetBuffer(uint numFramesRequested, out IntPtr dataBufferPointer);
    [PreserveSig] int ReleaseBuffer(uint numFramesWritten, uint flags);
}

[ComImport, Guid("C8ADBD64-E71E-48a0-A4DE-185C395CD317"),
 InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
public interface IAudioCaptureClient
{
    [PreserveSig] int GetBuffer(out IntPtr data, out uint numFramesToRead, out uint flags, out ulong devicePosition, out ulong qpcPosition);
    [PreserveSig] int ReleaseBuffer(uint numFramesRead);
    [PreserveSig] int GetNextPacketSize(out uint numFramesInNextPacket);
}

// --- Static helper class ---

public static class CableWasapi
{
    [DllImport("ole32.dll")]
    public static extern int PropVariantClear(ref PROPVARIANT pvar);

    [DllImport("ole32.dll")]
    public static extern void CoTaskMemFree(IntPtr ptr);

    // Not readonly — used with ref parameters in COM calls.
    public static Guid CLSID_MMDeviceEnumerator = new Guid("BCDE0395-E52F-467C-8E3D-C4579291692E");
    public static Guid IID_IAudioClient         = new Guid("1CB9AD4C-DBFA-4c32-B178-C2F568A703B2");
    public static Guid IID_IAudioRenderClient   = new Guid("F294ACFC-3146-4483-A7BF-ADDCA7C260E2");
    public static Guid IID_IAudioCaptureClient  = new Guid("C8ADBD64-E71E-48a0-A4DE-185C395CD317");

    public const uint AUDCLNT_SHAREMODE_SHARED          = 0;
    public const uint AUDCLNT_STREAMFLAGS_EVENTCALLBACK = 0x00040000;
    public const uint AUDCLNT_STREAMFLAGS_LOOPBACK      = 0x00020000;

    public const int STGM_READ      = 0x00000000;
    public const int STGM_READWRITE = 0x00000002;

    // --- Error handling ---

    public static void ThrowIfFailed(int hr, string op)
    {
        if (hr < 0) Marshal.ThrowExceptionForHR(hr, new IntPtr(-1));
    }

    // --- Property helpers ---

    public static PROPERTYKEY PkeyFriendlyName()
    {
        return new PROPERTYKEY
        {
            fmtid = new Guid("A45C254E-DF1C-4EFD-8020-67D146A850E0"),
            pid = 14
        };
    }

    public static PROPERTYKEY PkeyDeviceDesc()
    {
        return new PROPERTYKEY
        {
            fmtid = new Guid("A45C254E-DF1C-4EFD-8020-67D146A850E0"),
            pid = 2
        };
    }

    public static string ReadStringProp(IPropertyStore store, PROPERTYKEY key)
    {
        PROPVARIANT pv;
        int hr = store.GetValue(ref key, out pv);
        if (hr < 0 || pv.vt != 31 || pv.pointerValue == IntPtr.Zero)
        {
            PropVariantClear(ref pv);
            return string.Empty;
        }
        string s = Marshal.PtrToStringUni(pv.pointerValue) ?? string.Empty;
        PropVariantClear(ref pv);
        return s;
    }

    public static PROPVARIANT MakeStringPropVariant(string value)
    {
        PROPVARIANT pv = new PROPVARIANT();
        pv.vt = 31; // VT_LPWSTR
        pv.pointerValue = Marshal.StringToCoTaskMemUni(value);
        return pv;
    }

    // --- Device enumeration ---

    public static string ReadFriendlyName(IMMDevice device)
    {
        IPropertyStore store;
        ThrowIfFailed(device.OpenPropertyStore(STGM_READ, out store), "OpenPropertyStore");
        return ReadStringProp(store, PkeyFriendlyName());
    }

    public static IMMDevice FindDeviceByName(int dataFlow, string contains)
    {
        var enumerator = (IMMDeviceEnumerator)Activator.CreateInstance(
            Type.GetTypeFromCLSID(CLSID_MMDeviceEnumerator));
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

        return null;
    }

    public static IMMDevice FindDeviceByNameOrThrow(int dataFlow, string contains)
    {
        IMMDevice device = FindDeviceByName(dataFlow, contains);
        if (device == null)
            throw new InvalidOperationException(
                "Device not found: " + contains + " (dataFlow=" + dataFlow + ")");
        return device;
    }

    public static string ListAllDevices(int dataFlow)
    {
        var enumerator = (IMMDeviceEnumerator)Activator.CreateInstance(
            Type.GetTypeFromCLSID(CLSID_MMDeviceEnumerator));

        IMMDeviceCollection collection;
        ThrowIfFailed(enumerator.EnumAudioEndpoints(dataFlow, 0xF, out collection), "EnumAudioEndpoints(all)");

        uint count;
        ThrowIfFailed(collection.GetCount(out count), "GetCount(all)");

        var sb = new System.Text.StringBuilder();
        sb.Append("ALL_DEVICES(" + count + ",flow=" + dataFlow + "):");
        for (uint i = 0; i < count; i++)
        {
            IMMDevice device;
            ThrowIfFailed(collection.Item(i, out device), "Item");
            string name = "";
            try { name = ReadFriendlyName(device); } catch { name = "(error)"; }
            uint state;
            device.GetState(out state);
            string id;
            device.GetId(out id);
            sb.Append(" [" + i + "] '" + name + "' state=" + state + " id=" + (id ?? "?"));
        }
        return sb.ToString();
    }

    // --- Audio client helpers ---

    public static IAudioClient ActivateAudioClient(IMMDevice device)
    {
        IntPtr ptr;
        ThrowIfFailed(device.Activate(ref IID_IAudioClient, 23, IntPtr.Zero, out ptr),
            "IMMDevice::Activate(IAudioClient)");
        return (IAudioClient)Marshal.GetObjectForIUnknown(ptr);
    }

    public static IntPtr GetMixFormatPtr(IAudioClient client)
    {
        IntPtr pFormat;
        ThrowIfFailed(client.GetMixFormat(out pFormat), "IAudioClient::GetMixFormat");
        return pFormat;
    }
}
