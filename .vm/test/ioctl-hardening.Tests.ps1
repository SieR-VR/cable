# IOCTL hardening regression tests for CableAudio driver.
# Verifies STATUS_DEVICE_BUSY behavior and map/unmap/remove stability loops.

BeforeAll {
    . (Join-Path $PSScriptRoot "common.ps1")

    function script:Invoke-HardeningSuite {
        param(
            [System.Management.Automation.Runspaces.PSSession]$Session,
            [scriptblock]$Script
        )

        $ioctlCs = @'
using System;
using System.Runtime.InteropServices;
using Microsoft.Win32.SafeHandles;

public class CableHardeningIoctl
{
    const int DEVICE_CONTROL_PAYLOAD_SIZE = 662;

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

    static byte[] ParseId(string idHex)
    {
        string[] parts = idHex.Split('-');
        byte[] id = new byte[16];
        for (int i = 0; i < Math.Min(parts.Length, 16); i++)
        {
            id[i] = Convert.ToByte(parts[i], 16);
        }
        return id;
    }

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
        bool ok = DeviceIoControl(h, 0x80000004u, buf, (uint)buf.Length, outBuf, (uint)outBuf.Length, out bytesReturned, IntPtr.Zero);
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
        bool ok = DeviceIoControl(h, 0x80000008u, buf, (uint)buf.Length, outBuf, (uint)outBuf.Length, out bytesReturned, IntPtr.Zero);
        int err = Marshal.GetLastWin32Error();
        h.Close();

        if (!ok) return "REMOVE FAILED: error=" + err;
        return "REMOVE OK";
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
        bool ok = DeviceIoControl(h, 0x80000014u, inBuf, 16, outBuf, 16, out bytesReturned, IntPtr.Zero);
        int err = Marshal.GetLastWin32Error();
        h.Close();

        if (!ok) return "MAP FAILED: error=" + err;

        ulong address = BitConverter.ToUInt64(outBuf, 0);
        uint totalSize = BitConverter.ToUInt32(outBuf, 8);
        uint dataSize = BitConverter.ToUInt32(outBuf, 12);
        return "MAP OK: Address=0x" + address.ToString("X") + ",Total=" + totalSize + ",Data=" + dataSize;
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
        bool ok = DeviceIoControl(h, 0x80000018u, inBuf, 24, outBuf, (uint)outBuf.Length, out bytesReturned, IntPtr.Zero);
        int err = Marshal.GetLastWin32Error();
        h.Close();

        if (!ok) return "UNMAP FAILED: error=" + err;
        return "UNMAP OK";
    }
}
'@

        $helperFunctions = @'
function Parse-CreateId {
    param([string]$CreateResult)
    if ($CreateResult -notlike "CREATE OK: Id=*") {
        throw "Create failed: $CreateResult"
    }
    return $CreateResult.Substring("CREATE OK: Id=".Length)
}

function Parse-MapAddress {
    param([string]$MapResult)
    if ($MapResult -notlike "MAP OK: Address=0x*") {
        throw "Map failed: $MapResult"
    }

    $prefix = "MAP OK: Address=0x"
    $addrHex = ($MapResult.Substring($prefix.Length) -split ",")[0]
    return [UInt64]::Parse($addrHex, [System.Globalization.NumberStyles]::HexNumber)
}

function Invoke-BusyRemoveSequence {
    param(
        [int]$DeviceType,
        [string]$Name
    )

    $log = @()
    $create = [CableHardeningIoctl]::Create($DeviceType, $Name)
    $log += $create

    $id = Parse-CreateId -CreateResult $create

    $map = [CableHardeningIoctl]::MapRingBuffer($id)
    $log += $map

    $removeBusy = [CableHardeningIoctl]::Remove($id)
    $log += $removeBusy

    $address = Parse-MapAddress -MapResult $map
    $unmap = [CableHardeningIoctl]::UnmapRingBuffer($id, $address)
    $log += $unmap

    $removeFinal = [CableHardeningIoctl]::Remove($id)
    $log += $removeFinal

    return $log
}

function Invoke-WrongAddressUnmapSequence {
    param(
        [int]$DeviceType,
        [string]$Name
    )

    $log = @()
    $create = [CableHardeningIoctl]::Create($DeviceType, $Name)
    $log += $create

    $id = Parse-CreateId -CreateResult $create

    $map = [CableHardeningIoctl]::MapRingBuffer($id)
    $log += $map

    $address = Parse-MapAddress -MapResult $map
    $wrongAddress = $address + 0x1000

    $wrongUnmap = [CableHardeningIoctl]::UnmapRingBuffer($id, $wrongAddress)
    $log += $wrongUnmap

    $validUnmap = [CableHardeningIoctl]::UnmapRingBuffer($id, $address)
    $log += $validUnmap

    $remove = [CableHardeningIoctl]::Remove($id)
    $log += $remove

    return $log
}
'@

        $callerBody = $Script.ToString()
        $fullScript = '$ErrorActionPreference = "Stop"' + "`n" +
                      'Add-Type -TypeDefinition @''' + "`n" +
                      $ioctlCs + "`n" +
                      "'" + '@' + "`n`n" +
                      $helperFunctions + "`n`n" +
                      '& { ' + $callerBody + ' }'

        $output = Invoke-Command -Session $Session -ScriptBlock {
            param($body)
            $tmp = "C:\CableAudio\ioctl-hardening-suite.ps1"
            Set-Content -Path $tmp -Value $body -Encoding UTF8
            & powershell -NoProfile -ExecutionPolicy Bypass -File $tmp
        } -ArgumentList $fullScript

        return $output
    }
}

Describe "IOCTL: hardening remove busy policy" {
    BeforeAll {
        $script:Session = Reset-Vm @VmContext
    }

    AfterAll {
        if ($script:Session) {
            Assert-NoGuestBugCheck -ComputerName $VmContext.ComputerName -Port $VmContext.Port -Username $VmContext.Username -Password $VmContext.Password -Context "IOCTL hardening remove busy policy"
            Remove-PSSession $script:Session -ErrorAction SilentlyContinue
        }
    }

    It "returns busy while mapped and succeeds after unmap" {
        $output = Invoke-HardeningSuite -Session $script:Session -Script {
            Invoke-BusyRemoveSequence -DeviceType 0 -Name "Hardening Busy Remove"
        }

        $output | Should -Contain "UNMAP OK"
        $output | Should -Contain "REMOVE OK"
        ($output -join "`n") | Should -Match 'REMOVE FAILED: error=(170|2404)'
    }
}

Describe "IOCTL: hardening stress loop" {
    BeforeAll {
        $script:Session = Reset-Vm @VmContext
    }

    AfterAll {
        if ($script:Session) {
            Assert-NoGuestBugCheck -ComputerName $VmContext.ComputerName -Port $VmContext.Port -Username $VmContext.Username -Password $VmContext.Password -Context "IOCTL hardening stress loop"
            Remove-PSSession $script:Session -ErrorAction SilentlyContinue
        }
    }

    It "repeats busy-remove then unmap-remove cycles" {
        $loopCount = if ($VmContext.ContainsKey('RenameLoopCount')) {
            [Math]::Max(2, [int]$VmContext.RenameLoopCount)
        } else {
            3
        }

        $guestScript = @"
`$results = @()
`$loopCount = $loopCount

for (`$i = 0; `$i -lt `$loopCount; `$i++) {
    `$results += "LOOP `$i"
    `$results += Invoke-BusyRemoveSequence -DeviceType 1 -Name ("Hardening Stress " + `$i)
}

`$results
"@

        $output = Invoke-HardeningSuite -Session $script:Session -Script ([scriptblock]::Create($guestScript))

        ($output | Where-Object { $_ -eq 'REMOVE OK' }).Count | Should -BeGreaterThan 0
        ($output | Where-Object { $_ -eq 'UNMAP OK' }).Count | Should -BeGreaterThan 0
        ($output -join "`n") | Should -Not -Match 'ERR:OpenDriver|CREATE FAILED|MAP FAILED|UNMAP FAILED'
    }
}

Describe "IOCTL: hardening unmap address validation" {
    BeforeAll {
        $script:Session = Reset-Vm @VmContext
    }

    AfterAll {
        if ($script:Session) {
            Assert-NoGuestBugCheck -ComputerName $VmContext.ComputerName -Port $VmContext.Port -Username $VmContext.Username -Password $VmContext.Password -Context "IOCTL hardening unmap address validation"
            Remove-PSSession $script:Session -ErrorAction SilentlyContinue
        }
    }

    It "rejects wrong unmap address and still allows valid cleanup" {
        $output = Invoke-HardeningSuite -Session $script:Session -Script {
            Invoke-WrongAddressUnmapSequence -DeviceType 0 -Name "Hardening Wrong Unmap"
        }

        ($output -join "`n") | Should -Match 'UNMAP FAILED: error=87'
        $output | Should -Contain "UNMAP OK"
        $output | Should -Contain "REMOVE OK"
    }
}
