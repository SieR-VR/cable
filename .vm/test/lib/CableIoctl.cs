// Shared IOCTL interop for CableAudio driver VM tests.
// Provides low-level device control operations via DeviceIoControl.
//
// Used by: Ioctl.Tests.ps1, IoctlHardening.Tests.ps1,
//          RingBufferDataflow.Tests.ps1

using System;
using System.Globalization;
using System.Runtime.InteropServices;
using System.Threading;
using Microsoft.Win32.SafeHandles;

public static class CableIoctl
{
    public const int DEVICE_CONTROL_PAYLOAD_SIZE = 662;
    public const uint IOCTL_CREATE = 0x80000004u;
    public const uint IOCTL_REMOVE = 0x80000008u;
    public const uint IOCTL_RENAME = 0x8000000Cu;
    public const uint IOCTL_MAP    = 0x80000014u;
    public const uint IOCTL_UNMAP  = 0x80000018u;
    public const uint MAGIC_CBRB   = 0x42524243u;

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

    #endregion

    // --- Parsing helpers ---

    public static byte[] ParseId(string idHex)
    {
        string[] parts = idHex.Split('-');
        byte[] id = new byte[16];
        for (int i = 0; i < Math.Min(parts.Length, 16); i++)
            id[i] = Convert.ToByte(parts[i], 16);
        return id;
    }

    public static string ParseCreateId(string createResult)
    {
        if (createResult == null || !createResult.StartsWith("CREATE OK: Id="))
            throw new InvalidOperationException("Create failed: " + createResult);
        return createResult.Substring("CREATE OK: Id=".Length);
    }

    public static ulong ParseMapAddress(string mapResult)
    {
        if (mapResult == null || !mapResult.StartsWith("MAP OK: Address=0x"))
            throw new InvalidOperationException("Map failed: " + mapResult);
        string addrHex = (mapResult.Substring("MAP OK: Address=0x".Length).Split(','))[0];
        return UInt64.Parse(addrHex, NumberStyles.HexNumber);
    }

    public static uint ParseMapDataSize(string mapResult)
    {
        if (mapResult == null || !mapResult.Contains(",Data="))
            throw new InvalidOperationException("Map result missing Data field: " + mapResult);
        int idx = mapResult.IndexOf(",Data=") + 6;
        return uint.Parse(mapResult.Substring(idx));
    }

    // --- Driver handle ---

    public static SafeFileHandle OpenDriver()
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

    // --- IOCTL operations ---

    public static string Create(int deviceType, string friendlyName)
    {
        SafeFileHandle h = OpenDriver();
        if (h == null || h.IsInvalid) return "ERR:OpenDriver";

        byte[] buf = new byte[DEVICE_CONTROL_PAYLOAD_SIZE];
        byte[] name = System.Text.Encoding.Unicode.GetBytes(friendlyName);
        int copyLen = Math.Min(name.Length, 126);
        Array.Copy(name, 0, buf, 16, copyLen);
        buf[144] = (byte)(deviceType & 0xFF);
        buf[148] = 0x01;

        byte[] outBuf = new byte[DEVICE_CONTROL_PAYLOAD_SIZE];
        uint bytesReturned;
        bool ok = DeviceIoControl(h, IOCTL_CREATE, buf, (uint)buf.Length,
            outBuf, (uint)outBuf.Length, out bytesReturned, IntPtr.Zero);
        int err = Marshal.GetLastWin32Error();
        h.Close();

        if (!ok) return "CREATE FAILED: error=" + err;
        return "CREATE OK: Id=" + BitConverter.ToString(outBuf, 0, 16);
    }

    public static string Remove(string idHex)
    {
        SafeFileHandle h = OpenDriver();
        if (h == null || h.IsInvalid) return "ERR:OpenDriver";

        byte[] id = ParseId(idHex);
        byte[] buf = new byte[DEVICE_CONTROL_PAYLOAD_SIZE];
        Array.Copy(id, 0, buf, 0, 16);

        byte[] outBuf = new byte[DEVICE_CONTROL_PAYLOAD_SIZE];
        uint bytesReturned;
        bool ok = DeviceIoControl(h, IOCTL_REMOVE, buf, (uint)buf.Length,
            outBuf, (uint)outBuf.Length, out bytesReturned, IntPtr.Zero);
        int err = Marshal.GetLastWin32Error();
        h.Close();

        if (!ok) return "REMOVE FAILED: error=" + err;
        return "REMOVE OK";
    }

    public static string RemoveWithRetry(string idHex, int maxRetries, int retryDelayMs)
    {
        string result = "";
        for (int i = 0; i <= maxRetries; i++)
        {
            result = Remove(idHex);
            if (result == "REMOVE OK") return result;
            if (!result.Contains("error=170")) return result;
            if (i < maxRetries) Thread.Sleep(retryDelayMs);
        }
        return result;
    }

    public static string UpdateName(string idHex, string newName)
    {
        SafeFileHandle h = OpenDriver();
        if (h == null || h.IsInvalid) return "ERR:OpenDriver";

        byte[] id = ParseId(idHex);
        byte[] buf = new byte[DEVICE_CONTROL_PAYLOAD_SIZE];
        Array.Copy(id, 0, buf, 0, 16);

        byte[] name = System.Text.Encoding.Unicode.GetBytes(newName);
        int copyLen = Math.Min(name.Length, 126);
        Array.Copy(name, 0, buf, 16, copyLen);

        byte[] outBuf = new byte[DEVICE_CONTROL_PAYLOAD_SIZE];
        uint bytesReturned;
        bool ok = DeviceIoControl(h, IOCTL_RENAME, buf, (uint)buf.Length,
            outBuf, (uint)outBuf.Length, out bytesReturned, IntPtr.Zero);
        int err = Marshal.GetLastWin32Error();
        h.Close();

        if (!ok) return "RENAME FAILED: error=" + err;
        return "RENAME OK";
    }

    public static string MapRingBuffer(string idHex)
    {
        SafeFileHandle h = OpenDriver();
        if (h == null || h.IsInvalid) return "ERR:OpenDriver";

        byte[] id = ParseId(idHex);
        byte[] inBuf = new byte[16];
        Array.Copy(id, 0, inBuf, 0, 16);

        byte[] outBuf = new byte[16];
        uint bytesReturned;
        bool ok = DeviceIoControl(h, IOCTL_MAP, inBuf, 16, outBuf, 16,
            out bytesReturned, IntPtr.Zero);
        int err = Marshal.GetLastWin32Error();
        h.Close();

        if (!ok) return "MAP FAILED: error=" + err;

        ulong address = BitConverter.ToUInt64(outBuf, 0);
        uint totalSize = BitConverter.ToUInt32(outBuf, 8);
        uint dataSize = BitConverter.ToUInt32(outBuf, 12);
        return "MAP OK: Address=0x" + address.ToString("X") +
            ",Total=" + totalSize + ",Data=" + dataSize;
    }

    public static string UnmapRingBuffer(string idHex, ulong address)
    {
        SafeFileHandle h = OpenDriver();
        if (h == null || h.IsInvalid) return "ERR:OpenDriver";

        byte[] id = ParseId(idHex);
        byte[] inBuf = new byte[24];
        Array.Copy(id, 0, inBuf, 0, 16);
        Array.Copy(BitConverter.GetBytes(address), 0, inBuf, 16, 8);

        byte[] outBuf = new byte[4];
        uint bytesReturned;
        bool ok = DeviceIoControl(h, IOCTL_UNMAP, inBuf, 24, outBuf,
            (uint)outBuf.Length, out bytesReturned, IntPtr.Zero);
        int err = Marshal.GetLastWin32Error();
        h.Close();

        if (!ok) return "UNMAP FAILED: error=" + err;
        return "UNMAP OK";
    }
}
