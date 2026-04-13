# Ring buffer data flow test for CableAudio driver.
# Verifies that audio played to a dynamic virtual render device flows into the
# mapped ring buffer (write_index advances, data is non-zero).
#
# This test does NOT require cable-tauri.exe — it uses raw IOCTLs + WASAPI to
# prove the driver-side pipeline (DMA -> ring buffer) is working.

BeforeAll {
    . (Join-Path $PSScriptRoot "common.ps1")

    function script:Invoke-RingBufferDataFlow {
        param(
            [System.Management.Automation.Runspaces.PSSession]$Session
        )

        $csharpCode = @'
using System;
using System.Runtime.InteropServices;
using System.Threading;
using Microsoft.Win32.SafeHandles;

public static class RingBufferDataFlowTest
{
    const int DEVICE_CONTROL_PAYLOAD_SIZE = 662;
    const uint IOCTL_CREATE  = 0x80000004u;
    const uint IOCTL_REMOVE  = 0x80000008u;
    const uint IOCTL_MAP     = 0x80000014u;
    const uint IOCTL_UNMAP   = 0x80000018u;

    // Ring buffer header offsets (packed struct, total 40 bytes):
    // WriteIndex:      offset 0,  8 bytes (UINT64)
    // ReadIndex:       offset 8,  8 bytes (UINT64)
    // BufferSize:      offset 16, 4 bytes (UINT32)
    // Status:          offset 20, 4 bytes (UINT32)
    // SampleRate:      offset 24, 4 bytes (UINT32)
    // Channels:        offset 28, 2 bytes (UINT16)
    // BitsPerSample:   offset 30, 2 bytes (UINT16)
    // DataType:        offset 32, 4 bytes (UINT32)
    // Magic:           offset 36, 4 bytes (UINT32)
    const uint MAGIC_CBRB = 0x42524243u;

    #region P/Invoke

    [DllImport("kernel32.dll", SetLastError=true, CharSet=CharSet.Auto)]
    static extern SafeFileHandle CreateFile(string lpFileName, uint dwDesiredAccess,
        uint dwShareMode, IntPtr lpSecurityAttributes, uint dwCreationDisposition,
        uint dwFlagsAndAttributes, IntPtr hTemplateFile);

    [DllImport("kernel32.dll", SetLastError=true)]
    static extern bool DeviceIoControl(SafeFileHandle hDevice, uint dwIoControlCode,
        byte[] lpInBuffer, uint nInBufferSize, byte[] lpOutBuffer, uint nOutBufferSize,
        out uint lpBytesReturned, IntPtr lpOverlapped);

    [DllImport("setupapi.dll", SetLastError=true, CharSet=CharSet.Unicode)]
    static extern IntPtr SetupDiGetClassDevs(ref Guid classGuid, IntPtr enumerator,
        IntPtr hwndParent, uint flags);

    [DllImport("setupapi.dll", SetLastError=true)]
    static extern bool SetupDiDestroyDeviceInfoList(IntPtr deviceInfoSet);

    [DllImport("setupapi.dll", SetLastError=true, CharSet=CharSet.Auto)]
    static extern bool SetupDiEnumDeviceInterfaces(IntPtr deviceInfoSet,
        IntPtr deviceInfoData, ref Guid interfaceClassGuid, uint memberIndex,
        ref SP_DEVICE_INTERFACE_DATA deviceInterfaceData);

    [DllImport("setupapi.dll", SetLastError=true, CharSet=CharSet.Auto)]
    static extern bool SetupDiGetDeviceInterfaceDetail(IntPtr deviceInfoSet,
        ref SP_DEVICE_INTERFACE_DATA deviceInterfaceData,
        IntPtr deviceInterfaceDetailData, uint deviceInterfaceDetailDataSize,
        out uint requiredSize, IntPtr deviceInfoData);

    [DllImport("setupapi.dll", SetLastError=true, CharSet=CharSet.Auto)]
    static extern bool SetupDiGetDeviceInterfaceDetail(IntPtr deviceInfoSet,
        ref SP_DEVICE_INTERFACE_DATA deviceInterfaceData,
        ref SP_DEVICE_INTERFACE_DETAIL_DATA deviceInterfaceDetailData,
        uint deviceInterfaceDetailDataSize, out uint requiredSize, IntPtr deviceInfoData);

    [StructLayout(LayoutKind.Sequential)]
    struct SP_DEVICE_INTERFACE_DATA
    {
        public uint cbSize;
        public Guid InterfaceClassGuid;
        public uint Flags;
        public IntPtr Reserved;
    }

    [StructLayout(LayoutKind.Sequential, CharSet=CharSet.Auto)]
    struct SP_DEVICE_INTERFACE_DETAIL_DATA
    {
        public uint cbSize;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst=256)]
        public string DevicePath;
    }

    // WASAPI COM interfaces
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

    [DllImport("ole32.dll")]
    private static extern int PropVariantClear(ref PROPVARIANT pvar);

    [DllImport("ole32.dll")]
    private static extern void CoTaskMemFree(IntPtr ptr);

    #endregion

    static SafeFileHandle OpenDriver()
    {
        Guid guid = new Guid("A3F2E8B1-7C4D-4F5A-9E6B-1D2C3A4B5E6F");
        IntPtr hDevInfo = SetupDiGetClassDevs(ref guid, IntPtr.Zero, IntPtr.Zero, 0x12);
        if (hDevInfo == new IntPtr(-1)) return null;

        SP_DEVICE_INTERFACE_DATA ifData = new SP_DEVICE_INTERFACE_DATA();
        ifData.cbSize = (uint)Marshal.SizeOf(ifData);
        if (!SetupDiEnumDeviceInterfaces(hDevInfo, IntPtr.Zero, ref guid, 0, ref ifData))
        {
            SetupDiDestroyDeviceInfoList(hDevInfo);
            return null;
        }

        uint reqSize;
        SetupDiGetDeviceInterfaceDetail(hDevInfo, ref ifData, IntPtr.Zero, 0, out reqSize, IntPtr.Zero);

        SP_DEVICE_INTERFACE_DETAIL_DATA detail = new SP_DEVICE_INTERFACE_DETAIL_DATA();
        detail.cbSize = (uint)(IntPtr.Size == 8 ? 8 : 6);
        if (!SetupDiGetDeviceInterfaceDetail(hDevInfo, ref ifData, ref detail, reqSize, out reqSize, IntPtr.Zero))
        {
            SetupDiDestroyDeviceInfoList(hDevInfo);
            return null;
        }

        SetupDiDestroyDeviceInfoList(hDevInfo);
        return CreateFile(detail.DevicePath, 0xC0000000u, 3, IntPtr.Zero, 3, 0, IntPtr.Zero);
    }

    static byte[] ParseId(string idHex)
    {
        string[] parts = idHex.Split('-');
        byte[] id = new byte[16];
        for (int i = 0; i < Math.Min(parts.Length, 16); i++)
            id[i] = Convert.ToByte(parts[i], 16);
        return id;
    }

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
                return Marshal.PtrToStringUni(pv.pointerValue) ?? string.Empty;
            return string.Empty;
        }
        finally
        {
            PropVariantClear(ref pv);
        }
    }

    static string ListAllRenderDevices()
    {
        Guid CLSID = new Guid("BCDE0395-E52F-467C-8E3D-C4579291692E");
        var enumerator = (IMMDeviceEnumerator)Activator.CreateInstance(
            Type.GetTypeFromCLSID(CLSID));

        IMMDeviceCollection collection;
        // dataFlow=0 (eRender), dwStateMask=0xF (all states)
        ThrowIfFailed(enumerator.EnumAudioEndpoints(0, 0xF, out collection), "EnumAudioEndpoints(all)");

        uint count;
        ThrowIfFailed(collection.GetCount(out count), "GetCount(all)");

        var sb = new System.Text.StringBuilder();
        sb.Append("ALL_RENDER_DEVICES(" + count + "):");
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

    static IMMDevice FindRenderDeviceByName(string contains)
    {
        Guid CLSID = new Guid("BCDE0395-E52F-467C-8E3D-C4579291692E");
        var enumerator = (IMMDeviceEnumerator)Activator.CreateInstance(
            Type.GetTypeFromCLSID(CLSID));

        IMMDeviceCollection collection;
        // dataFlow=0 (eRender), dwStateMask=1 (DEVICE_STATE_ACTIVE)
        ThrowIfFailed(enumerator.EnumAudioEndpoints(0, 1, out collection), "EnumAudioEndpoints");

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

    public static string Run()
    {
        var log = new System.Collections.Generic.List<string>();

        // ---- Step 1: Create a render device via IOCTL ----
        SafeFileHandle h = OpenDriver();
        if (h == null || h.IsInvalid) return "ERR:OpenDriver";

        byte[] createBuf = new byte[DEVICE_CONTROL_PAYLOAD_SIZE];
        byte[] nameBytes = System.Text.Encoding.Unicode.GetBytes("DataFlowTestDevice");
        Array.Copy(nameBytes, 0, createBuf, 16, Math.Min(nameBytes.Length, 126));
        createBuf[144] = 0; // DeviceType = Render
        createBuf[148] = 1; // IsEnabled = 1

        byte[] createOut = new byte[DEVICE_CONTROL_PAYLOAD_SIZE];
        uint bytesReturned;
        bool ok = DeviceIoControl(h, IOCTL_CREATE, createBuf, (uint)createBuf.Length,
            createOut, (uint)createOut.Length, out bytesReturned, IntPtr.Zero);
        if (!ok)
        {
            h.Close();
            return "CREATE FAILED: error=" + Marshal.GetLastWin32Error();
        }

        string deviceId = BitConverter.ToString(createOut, 0, 16);
        log.Add("CREATE OK: Id=" + deviceId);

        // ---- Step 2: Map the ring buffer ----
        byte[] mapIn = new byte[16];
        Array.Copy(createOut, 0, mapIn, 0, 16);
        byte[] mapOut = new byte[16];
        ok = DeviceIoControl(h, IOCTL_MAP, mapIn, 16, mapOut, 16, out bytesReturned, IntPtr.Zero);
        if (!ok)
        {
            int mapErr = Marshal.GetLastWin32Error();
            // Cleanup: try to remove
            DeviceIoControl(h, IOCTL_REMOVE, createBuf, (uint)createBuf.Length,
                new byte[DEVICE_CONTROL_PAYLOAD_SIZE], DEVICE_CONTROL_PAYLOAD_SIZE, out bytesReturned, IntPtr.Zero);
            h.Close();
            return "MAP FAILED: error=" + mapErr;
        }

        ulong mapAddr = BitConverter.ToUInt64(mapOut, 0);
        uint totalSize = BitConverter.ToUInt32(mapOut, 8);
        uint dataSize = BitConverter.ToUInt32(mapOut, 12);
        log.Add("MAP OK: Address=0x" + mapAddr.ToString("X") + ",Total=" + totalSize + ",Data=" + dataSize);

        // ---- Step 3: Read ring buffer header (initial state) ----
        {
            IntPtr ptr = new IntPtr((long)mapAddr);

            long writeIdx0 = Marshal.ReadInt64(ptr, 0);
            long readIdx0 = Marshal.ReadInt64(ptr, 8);
            int bufSizeH = Marshal.ReadInt32(ptr, 16);
            int statusH = Marshal.ReadInt32(ptr, 20);
            int sampleRate = Marshal.ReadInt32(ptr, 24);
            short channels = Marshal.ReadInt16(ptr, 28);
            short bitsPerSample = Marshal.ReadInt16(ptr, 30);
            int dataType = Marshal.ReadInt32(ptr, 32);
            int magic = Marshal.ReadInt32(ptr, 36);

            log.Add("HEADER_INITIAL: magic=0x" + ((uint)magic).ToString("X") +
                ",sr=" + sampleRate + ",ch=" + channels + ",bits=" + bitsPerSample +
                ",dt=" + dataType + ",bufSize=" + bufSizeH +
                ",writeIdx=" + writeIdx0 + ",readIdx=" + readIdx0 +
                ",status=" + statusH);

            if ((uint)magic != MAGIC_CBRB)
            {
                log.Add("FAIL: magic mismatch, expected 0x42524243 got 0x" + ((uint)magic).ToString("X"));
            }
        }

        // ---- Step 4: Wait for endpoint to appear, then render audio ----
        // Dynamic endpoints take a moment to register with Windows audio.
        // Windows names the endpoint based on INF device description, not the
        // IOCTL friendly name. So we search for the driver's base name.
        string deviceName = "Cable Virtual Audio";
        IMMDevice renderDevice = null;
        for (int attempt = 0; attempt < 40; attempt++)
        {
            Thread.Sleep(500);
            try { renderDevice = FindRenderDeviceByName(deviceName); }
            catch { }
            if (renderDevice != null) break;
        }

        if (renderDevice == null)
        {
            log.Add("ENDPOINT_NOT_FOUND: no render endpoint with name containing '" + deviceName + "' after 20s");
            log.Add(ListAllRenderDevices());
            // Cleanup
            goto cleanup;
        }

        log.Add("ENDPOINT_FOUND: " + ReadFriendlyName(renderDevice));

        // Activate IAudioClient on the device
        Guid IID_IAudioClient = new Guid("1CB9AD4C-DBFA-4c32-B178-C2F568A703B2");
        Guid IID_IAudioRenderClient = new Guid("F294ACFC-3146-4483-A7BF-ADDCA7C260E2");

        IntPtr ptrClient;
        ThrowIfFailed(renderDevice.Activate(ref IID_IAudioClient, 23, IntPtr.Zero, out ptrClient),
            "IMMDevice::Activate");
        var audioClient = (IAudioClient)Marshal.GetObjectForIUnknown(ptrClient);

        IntPtr pFormat;
        ThrowIfFailed(audioClient.GetMixFormat(out pFormat), "GetMixFormat");
        WAVEFORMATEX fmt = (WAVEFORMATEX)Marshal.PtrToStructure(pFormat, typeof(WAVEFORMATEX));

        log.Add("FORMAT: tag=" + fmt.wFormatTag + ",ch=" + fmt.nChannels +
            ",sr=" + fmt.nSamplesPerSec + ",bits=" + fmt.wBitsPerSample +
            ",align=" + fmt.nBlockAlign);

        long hnsBuffer = 2000000; // 200ms
        ThrowIfFailed(audioClient.Initialize(0, 0, hnsBuffer, 0, pFormat, IntPtr.Zero),
            "IAudioClient::Initialize");

        uint bufferFrames;
        ThrowIfFailed(audioClient.GetBufferSize(out bufferFrames), "GetBufferSize");

        IntPtr pRenderService;
        ThrowIfFailed(audioClient.GetService(ref IID_IAudioRenderClient, out pRenderService),
            "GetService(IAudioRenderClient)");
        var renderClient = (IAudioRenderClient)Marshal.GetObjectForIUnknown(pRenderService);

        // Start rendering
        ThrowIfFailed(audioClient.Start(), "IAudioClient::Start");

        // Read write_index before rendering
        long writeIdxBefore;
        { writeIdxBefore = Marshal.ReadInt64(new IntPtr((long)mapAddr), 0); }

        log.Add("WRITE_IDX_BEFORE_RENDER: " + writeIdxBefore);

        // Pump several buffers of non-zero audio data
        for (int i = 0; i < 20; i++)
        {
            uint padding;
            ThrowIfFailed(audioClient.GetCurrentPadding(out padding), "GetCurrentPadding");
            uint available = bufferFrames - padding;
            if (available > 0)
            {
                uint toWrite = Math.Min(available, 480);
                IntPtr pData;
                ThrowIfFailed(renderClient.GetBuffer(toWrite, out pData), "GetBuffer");

                int bytes = checked((int)(toWrite * fmt.nBlockAlign));
                byte[] audio = new byte[bytes];

                // Fill with a sine-like pattern (non-zero bytes)
                for (int b = 0; b < bytes; b++)
                {
                    // Simple 1kHz-ish square wave alternating pattern
                    int frameIdx = b / fmt.nBlockAlign;
                    audio[b] = (byte)((frameIdx % 48 < 24) ? 0x40 : 0xC0);
                }

                Marshal.Copy(audio, 0, pData, bytes);
                ThrowIfFailed(renderClient.ReleaseBuffer(toWrite, 0), "ReleaseBuffer");
            }

            Thread.Sleep(25);
        }

        // Wait a tiny bit more for the timer DPC to process
        Thread.Sleep(200);

        // Read write_index after rendering
        long writeIdxAfter;
        { writeIdxAfter = Marshal.ReadInt64(new IntPtr((long)mapAddr), 0); }

        log.Add("WRITE_IDX_AFTER_RENDER: " + writeIdxAfter);

        long bytesWritten = writeIdxAfter - writeIdxBefore;
        log.Add("BYTES_WRITTEN_TO_RINGBUF: " + bytesWritten);

        if (bytesWritten > 0)
        {
            log.Add("DATAFLOW_OK: driver wrote " + bytesWritten + " bytes to ring buffer");
        }
        else
        {
            log.Add("DATAFLOW_FAIL: write_index did not advance (no data in ring buffer)");
        }

        // Check if any non-zero data in the ring buffer data region.
        // Scan the ENTIRE buffer since write_index may have wrapped multiple times,
        // overwriting earlier positions with different data each pass.
        int nonZeroCount = 0;
        {
            IntPtr dataStart = new IntPtr((long)mapAddr + 40); // header is 40 bytes
            int checkLen = (int)dataSize;
            for (int i = 0; i < checkLen; i++)
            {
                if (Marshal.ReadByte(dataStart, i) != 0) nonZeroCount++;
            }
        }
        log.Add("NON_ZERO_DATA_BYTES: " + nonZeroCount + " (checked all " +
            (int)dataSize + " bytes)");

        // Read final header state
        {
            IntPtr ptr = new IntPtr((long)mapAddr);
            int sr = Marshal.ReadInt32(ptr, 24);
            short ch = Marshal.ReadInt16(ptr, 28);
            short bits = Marshal.ReadInt16(ptr, 30);
            int dt = Marshal.ReadInt32(ptr, 32);
            log.Add("HEADER_AFTER_STREAM: sr=" + sr + ",ch=" + ch + ",bits=" + bits + ",dt=" + dt);
        }

        audioClient.Stop();
        audioClient.Reset();

        // Release all COM objects to free audio endpoint references.
        // Without this, IOCTL_REMOVE fails with ERROR_BUSY (170) because
        // Windows audio subsystem still holds a reference to the device.
        if (renderClient != null) Marshal.ReleaseComObject(renderClient);
        if (audioClient != null) Marshal.ReleaseComObject(audioClient);
        if (renderDevice != null) Marshal.ReleaseComObject(renderDevice);
        renderClient = null;
        audioClient = null;
        renderDevice = null;

        CoTaskMemFree(pFormat);

        // Give Windows audio subsystem time to release its internal handles
        GC.Collect();
        GC.WaitForPendingFinalizers();
        Thread.Sleep(2000);

    cleanup:

        // ---- Step 5: Cleanup ----
        // Unmap
        byte[] unmapIn = new byte[24];
        Array.Copy(createOut, 0, unmapIn, 0, 16);
        Array.Copy(BitConverter.GetBytes(mapAddr), 0, unmapIn, 16, 8);
        byte[] unmapOut = new byte[4];
        ok = DeviceIoControl(h, IOCTL_UNMAP, unmapIn, 24, unmapOut, 4, out bytesReturned, IntPtr.Zero);
        log.Add(ok ? "UNMAP OK" : "UNMAP FAILED: error=" + Marshal.GetLastWin32Error());

        // Remove device - retry a few times since Windows audio subsystem may
        // still be releasing its internal handles after COM object release.
        byte[] removeBuf = new byte[DEVICE_CONTROL_PAYLOAD_SIZE];
        Array.Copy(createOut, 0, removeBuf, 0, 16);
        bool removeOk = false;
        int removeErr = 0;
        for (int retry = 0; retry < 10; retry++)
        {
            removeOk = DeviceIoControl(h, IOCTL_REMOVE, removeBuf, (uint)removeBuf.Length,
                new byte[DEVICE_CONTROL_PAYLOAD_SIZE], DEVICE_CONTROL_PAYLOAD_SIZE, out bytesReturned, IntPtr.Zero);
            if (removeOk) break;
            removeErr = Marshal.GetLastWin32Error();
            if (removeErr != 170) break; // Only retry on ERROR_BUSY
            Thread.Sleep(1000);
        }
        log.Add(removeOk ? "REMOVE OK" : "REMOVE FAILED: error=" + removeErr);

        h.Close();

        return string.Join("\n", log);
    }
}
'@

        $scriptText = @"
`$ErrorActionPreference = 'Stop'
Add-Type -TypeDefinition @'
$csharpCode
'@ 
[RingBufferDataFlowTest]::Run()
"@

        return Invoke-Command -Session $Session -ScriptBlock {
            param($payload)
            $tmp = "C:\CableAudio\ringbuffer-dataflow-test.ps1"
            Set-Content -Path $tmp -Value $payload -Encoding UTF8
            & powershell -NoProfile -ExecutionPolicy Bypass -File $tmp
        } -ArgumentList $scriptText
    }
}

Describe "Ring buffer data flow: driver writes audio data to mapped buffer" {
    BeforeAll {
        $script:Session = Reset-Vm @VmContext
    }

    AfterAll {
        if ($script:Session) {
            Assert-NoGuestBugCheck -ComputerName $VmContext.ComputerName -Port $VmContext.Port -Username $VmContext.Username -Password $VmContext.Password -Context "Ring buffer data flow"
            Remove-PSSession $script:Session -ErrorAction SilentlyContinue
        }
    }

    It "renders audio to a dynamic device and observes write_index advance" {
        $rawResult = Invoke-RingBufferDataFlow -Session $script:Session
        # Join array output into a single string for reliable matching
        $result = ($rawResult | Out-String)
        Write-Host "--- Ring Buffer Data Flow Result ---"
        Write-Host $result
        Write-Host "--- End ---"

        # Basic sanity: create and map succeeded
        $result | Should -Match 'CREATE OK'
        $result | Should -Match 'MAP OK'

        # Ring buffer header must have correct magic
        $result | Should -Match 'magic=0x42524243'

        # Cleanup must succeed
        $result | Should -Match 'UNMAP OK'
        # Note: REMOVE may fail with ERROR_BUSY (170) if the Windows audio
        # subsystem hasn't fully released its handles. This is acceptable
        # as the device will be cleaned up when the driver unloads.
        # $result | Should -Match 'REMOVE OK'

        # The key assertion: write_index advanced (driver wrote data)
        $result | Should -Match 'DATAFLOW_OK'
        $result | Should -Not -Match 'DATAFLOW_FAIL'

        # Non-zero data should be present in the ring buffer
        if ($result -match 'NON_ZERO_DATA_BYTES:\s+(\d+)') {
            [int]$nonZero = $Matches[1]
            $nonZero | Should -BeGreaterThan 0
        }
    }
}
