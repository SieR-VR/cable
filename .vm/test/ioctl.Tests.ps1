# IOCTL test suite for CableAudio driver.
# Each Describe block resets the VM to a clean snapshot and installs the driver
# before running its It blocks.
#
# Requires: $VmContext hashtable in the caller's scope, injected via Pester's
# -Data parameter or script-scoped variable set by test.ps1 BeforeAll.

BeforeAll {
    . (Join-Path $PSScriptRoot "common.ps1")

    # ---------------------------------------------------------------------------
    # Helper: runs an inline script block inside the guest using the IOCTL C# type.
    # The scriptblock receives the Add-Type + helper functions already defined.
    # Defined inside BeforeAll so it is visible to all It blocks in this file.
    # ---------------------------------------------------------------------------
    function script:Invoke-IoctlSuite {
        param(
            [System.Management.Automation.Runspaces.PSSession]$Session,
            [scriptblock]$Script
        )

        $ioctlCs = @'
using System;
using System.Runtime.InteropServices;
using Microsoft.Win32.SafeHandles;

public class CableIoctlSuite
{
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

    public static string Create(int deviceType, string friendlyName)
    {
        SafeFileHandle h = OpenDriver();
        if (h == null || h.IsInvalid) return "ERR:OpenDriver";

        byte[] buf = new byte[150];
        byte[] name = System.Text.Encoding.Unicode.GetBytes(friendlyName);
        int copyLen = Math.Min(name.Length, 126);
        Array.Copy(name, 0, buf, 16, copyLen);
        buf[144] = (byte)(deviceType & 0xFF);
        buf[148] = 0x01;

        byte[] outBuf = new byte[150];
        uint bytesReturned;
        bool ok = DeviceIoControl(h, 0x80000004u, buf, 150, outBuf, 150, out bytesReturned, IntPtr.Zero);
        int err = Marshal.GetLastWin32Error();
        h.Close();

        if (!ok) return "CREATE FAILED: error=" + err;
        return "CREATE OK: Id=" + BitConverter.ToString(outBuf, 0, 16);
    }

    public static string Remove(string idHex)
    {
        SafeFileHandle h = OpenDriver();
        if (h == null || h.IsInvalid) return "ERR:OpenDriver";

        string[] parts = idHex.Split('-');
        byte[] id = new byte[16];
        for (int i = 0; i < Math.Min(parts.Length, 16); i++)
            id[i] = Convert.ToByte(parts[i], 16);

        byte[] buf = new byte[150];
        Array.Copy(id, 0, buf, 0, 16);

        byte[] outBuf = new byte[150];
        uint bytesReturned;
        bool ok = DeviceIoControl(h, 0x80000008u, buf, 150, outBuf, 150, out bytesReturned, IntPtr.Zero);
        int err = Marshal.GetLastWin32Error();
        h.Close();

        if (!ok) return "REMOVE FAILED: error=" + err;
        return "REMOVE OK";
    }

    public static string UpdateName(string idHex, string newName)
    {
        SafeFileHandle h = OpenDriver();
        if (h == null || h.IsInvalid) return "ERR:OpenDriver";

        string[] parts = idHex.Split('-');
        byte[] buf = new byte[150];
        for (int i = 0; i < Math.Min(parts.Length, 16); i++)
            buf[i] = Convert.ToByte(parts[i], 16);

        byte[] name = System.Text.Encoding.Unicode.GetBytes(newName);
        int copyLen = Math.Min(name.Length, 126);
        Array.Copy(name, 0, buf, 16, copyLen);

        byte[] outBuf = new byte[150];
        uint bytesReturned;
        bool ok = DeviceIoControl(h, 0x8000000Cu, buf, 150, outBuf, 150, out bytesReturned, IntPtr.Zero);
        int err = Marshal.GetLastWin32Error();
        h.Close();

        if (!ok) return "RENAME FAILED: error=" + err;
        return "RENAME OK";
    }
}
'@

        $helperFunctions = @'
function Run-CreateRemove {
    param([int]$DeviceType, [string]$Name)
    $create = [CableIoctlSuite]::Create($DeviceType, $Name)
    if ($create -notlike "CREATE OK: Id=*") { return @($create) }
    $id = $create.Substring("CREATE OK: Id=".Length)
    $remove = [CableIoctlSuite]::Remove($id)
    return @($create, $remove)
}

'@

        $callerBody = $Script.ToString()

        # Build the full guest script using string concatenation to avoid here-string nesting.
        $fullScript = '$ErrorActionPreference = "Stop"' + "`n" +
                      'Add-Type -TypeDefinition @''' + "`n" +
                      $ioctlCs + "`n" +
                      "'" + '@' + "`n`n" +
                      $helperFunctions + "`n`n" +
                      '& { ' + $callerBody + ' }'

        $output = Invoke-Command -Session $Session -ScriptBlock {
            param($body)
            $tmp = "C:\CableAudio\ioctl-suite.ps1"
            Set-Content -Path $tmp -Value $body -Encoding UTF8
            & powershell -NoProfile -ExecutionPolicy Bypass -File $tmp
        } -ArgumentList $fullScript

        return $output
    }
}

Describe "IOCTL: Capture device create/remove" {
    BeforeAll {
        $script:Session = Reset-Vm @VmContext
    }

    AfterAll {
        if ($script:Session) {
            Remove-PSSession $script:Session -ErrorAction SilentlyContinue
        }
    }

    It "creates a capture (mic) device and removes it cleanly" {
        $output = Invoke-IoctlSuite -Session $script:Session -Script {
            $results = @()
            $results += Run-CreateRemove -DeviceType 1 -Name "IOCTL Mic Test"
            $results
        }

        $output | Should -Not -Match 'FAILED|ERR:'
        $output | Should -Contain "REMOVE OK"
    }
}

Describe "IOCTL: Render device create/remove" {
    BeforeAll {
        $script:Session = Reset-Vm @VmContext
    }

    AfterAll {
        if ($script:Session) {
            Remove-PSSession $script:Session -ErrorAction SilentlyContinue
        }
    }

    It "creates a render (speaker) device and removes it cleanly" {
        $output = Invoke-IoctlSuite -Session $script:Session -Script {
            $results = @()
            $results += Run-CreateRemove -DeviceType 0 -Name "IOCTL Speaker Test"
            $results
        }

        $output | Should -Not -Match 'FAILED|ERR:'
        $output | Should -Contain "REMOVE OK"
    }
}

