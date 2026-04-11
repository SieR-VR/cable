# PKEY FriendlyName test for CableAudio driver.
# Uses IPropertyStore::SetValue (PKEY_Device_DeviceDesc / pid=2) to rename an
# audio endpoint's description component and verifies the change is reflected
# by the PnP layer.
#
# Background: PKEY_Device_FriendlyName (pid=14) is read-only for non-audio-service
# processes — the COM server (AudioEndpointBuilder) denies SetValue for that key.
# However, PKEY_Device_DeviceDesc (pid=2) is writable via IPropertyStore and drives
# the first component of the computed FriendlyName:
#   FriendlyName = "{DeviceDesc} ({DeviceInterface_FriendlyName})"
# e.g. "Speakers (Cable Virtual Audio Device)"
# Writing pid=2 = "PKEY Rename Test" makes pid=14 return
#   "PKEY Rename Test (Cable Virtual Audio Device)"
# which is also what Get-PnpDevice reports as the FriendlyName.
#
# Execution strategy: write and execute the rename C# snippet via WinRM
# (the cable/cable123 admin account has sufficient access).

BeforeAll {
    . (Join-Path $PSScriptRoot "common.ps1")

    # ---------------------------------------------------------------------------
    # Helper: runs the IPropertyStore rename C# snippet in the guest via WinRM.
    #
    # Strategy:
    #   1. Write a self-contained C# source file to C:\CableAudio\ in the guest
    #      via the existing WinRM session.
    #   2. Compile and execute it inside the same Invoke-Command call.
    #   3. Return the result string written by the snippet.
    #
    # The rename uses PKEY_Device_DeviceDesc (pid=2) because PKEY_Device_FriendlyName
    # (pid=14) is blocked by the AudioEndpointBuilder COM server for all non-service
    # processes.  Writing pid=2 immediately updates what pid=14 returns (the full
    # computed FriendlyName) and is reflected in Get-PnpDevice output.
    #
    # Returns a result string of the form:
    #   "EndpointId=... | Before='...' | After='...'"
    # ---------------------------------------------------------------------------
    function script:Invoke-PkeyRename {
        param(
            [System.Management.Automation.Runspaces.PSSession]$Session,
            [string]$TargetContains,
            [string]$NewName
        )

        # ------------------------------------------------------------------
        # C# source: uses IPropertyStore to rename the first matching render
        # endpoint by writing PKEY_Device_DeviceDesc (pid=2).
        # GetFriendlyName reads PKEY_Device_FriendlyName (pid=14) which is
        # computed as "{pid2} ({device interface name})" by the audio stack.
        # All COM interfaces use [PreserveSig] so HRESULTs are returned as
        # int instead of being auto-converted to exceptions.
        # ------------------------------------------------------------------
        $csCode = @'
using System;
using System.Runtime.InteropServices;

public static class EndpointRenameApi
{
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

    [ComImport, Guid("D666063F-1587-4E43-81F1-B948E807363F"),
     InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    public interface IMMDevice
    {
        [PreserveSig] int Activate(ref Guid iid, uint dwClsCtx, IntPtr pActivationParams, out IntPtr ppInterface);
        [PreserveSig] int OpenPropertyStore(int stgmAccess, out IPropertyStore ppProperties);
        [PreserveSig] int GetId([MarshalAs(UnmanagedType.LPWStr)] out string ppstrId);
        [PreserveSig] int GetState(out uint pdwState);
    }

    [ComImport, Guid("0BD7A1BE-7A1A-44DB-8397-CC5392387B5E"),
     InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    public interface IMMDeviceCollection
    {
        [PreserveSig] int GetCount(out uint pcDevices);
        [PreserveSig] int Item(uint nDevice, out IMMDevice ppDevice);
    }

    [ComImport, Guid("A95664D2-9614-4F35-A746-DE8DB63617E6"),
     InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    public interface IMMDeviceEnumerator
    {
        [PreserveSig] int EnumAudioEndpoints(int dataFlow, uint dwStateMask, out IMMDeviceCollection ppDevices);
        [PreserveSig] int GetDefaultAudioEndpoint(int dataFlow, int role, out IMMDevice ppEndpoint);
        [PreserveSig] int GetDevice(string pwstrId, out IMMDevice ppDevice);
        [PreserveSig] int RegisterEndpointNotificationCallback(IntPtr pClient);
        [PreserveSig] int UnregisterEndpointNotificationCallback(IntPtr pClient);
    }

    [DllImport("ole32.dll")]
    private static extern int PropVariantClear(ref PROPVARIANT pvar);

    private static readonly Guid CLSID_MMDeviceEnumerator =
        new Guid("BCDE0395-E52F-467C-8E3D-C4579291692E");

    private const int STGM_READ      = 0x00000000;
    private const int STGM_READWRITE = 0x00000002;

    // PKEY_Device_FriendlyName  = {A45C254E-DF1C-4EFD-8020-67D146A850E0}, pid 14
    // Read-only via IPropertyStore for non-audio-service processes.
    // Used only for reading the before/after state.
    private static PROPERTYKEY PkeyFriendlyName()
    {
        PROPERTYKEY k = new PROPERTYKEY();
        k.fmtid = new Guid("A45C254E-DF1C-4EFD-8020-67D146A850E0");
        k.pid   = 14;
        return k;
    }

    // PKEY_Device_DeviceDesc = {A45C254E-DF1C-4EFD-8020-67D146A850E0}, pid 2
    // Writable via IPropertyStore; drives the first component of FriendlyName.
    private static PROPERTYKEY PkeyDeviceDesc()
    {
        PROPERTYKEY k = new PROPERTYKEY();
        k.fmtid = new Guid("A45C254E-DF1C-4EFD-8020-67D146A850E0");
        k.pid   = 2;
        return k;
    }

    private static string ReadStringProp(IPropertyStore store, PROPERTYKEY key)
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

    private static PROPVARIANT MakeStringPropVariant(string value)
    {
        PROPVARIANT pv = new PROPVARIANT();
        pv.vt           = 31; // VT_LPWSTR
        pv.pointerValue = Marshal.StringToCoTaskMemUni(value);
        return pv;
    }

    private static void ThrowIfFailed(int hr, string op)
    {
        if (hr < 0) Marshal.ThrowExceptionForHR(hr);
    }

    public static string RenameFirstMatching(string contains, string newName)
    {
        object o = Activator.CreateInstance(
            Type.GetTypeFromCLSID(CLSID_MMDeviceEnumerator));
        IMMDeviceEnumerator enumerator = (IMMDeviceEnumerator)o;

        IMMDeviceCollection devices;
        ThrowIfFailed(enumerator.EnumAudioEndpoints(0, 1, out devices),
                      "EnumAudioEndpoints");

        uint count;
        ThrowIfFailed(devices.GetCount(out count), "GetCount");

        IMMDevice target    = null;
        string    original  = string.Empty;
        string    endpointId = string.Empty;

        for (uint i = 0; i < count; i++)
        {
            IMMDevice d;
            ThrowIfFailed(devices.Item(i, out d), "Item");

            IPropertyStore psRead;
            ThrowIfFailed(d.OpenPropertyStore(STGM_READ, out psRead),
                          "OpenPropertyStore(read)");

            string name = ReadStringProp(psRead, PkeyFriendlyName());

            if (!string.IsNullOrEmpty(contains) &&
                name.IndexOf(contains, StringComparison.OrdinalIgnoreCase) >= 0)
            {
                target     = d;
                original   = name;
                ThrowIfFailed(d.GetId(out endpointId), "GetId");
                break;
            }
        }

        if (target == null)
            throw new InvalidOperationException(
                "No matching endpoint found for: " + contains);

        // Open RW store and write PKEY_Device_DeviceDesc (pid=2).
        // pid=14 (FriendlyName) is blocked by the COM server for non-service
        // processes; pid=2 is writable and drives the computed FriendlyName.
        IPropertyStore storeRW;
        ThrowIfFailed(target.OpenPropertyStore(STGM_READWRITE, out storeRW),
                      "OpenPropertyStore(readwrite)");

        PROPVARIANT pv  = MakeStringPropVariant(newName);
        PROPERTYKEY key = PkeyDeviceDesc();
        try
        {
            ThrowIfFailed(storeRW.SetValue(ref key, ref pv), "SetValue(DeviceDesc)");
            ThrowIfFailed(storeRW.Commit(), "Commit");
        }
        finally
        {
            if (pv.pointerValue != IntPtr.Zero)
                Marshal.FreeCoTaskMem(pv.pointerValue);
        }

        // Re-read FriendlyName (pid=14) which is now computed as
        // "{newName} ({device interface name})" e.g.
        // "PKEY Rename Test (Cable Virtual Audio Device)"
        IPropertyStore storeRead;
        ThrowIfFailed(target.OpenPropertyStore(STGM_READ, out storeRead),
                      "OpenPropertyStore(read-after)");
        string after = ReadStringProp(storeRead, PkeyFriendlyName());

        return "EndpointId=" + endpointId +
               " | Before='" + original  + "'" +
               " | After='"  + after     + "'";
    }
}
'@

        $guestCsPath     = 'C:\CableAudio\pkey-rename-cs.txt'
        $guestResultPath = 'C:\CableAudio\pkey-rename-result.txt'

        # Remove any stale result file.
        Invoke-Command -Session $Session -ScriptBlock {
            param($p) if (Test-Path $p) { Remove-Item $p -Force }
        } -ArgumentList $guestResultPath

        # Write C# source to the guest, compile it, and run the rename — all
        # inside a single Invoke-Command so no vmrun or session juggling needed.
        Write-Host "  [PKEY] Executing rename via WinRM..." -ForegroundColor DarkCyan
        $renameResult = Invoke-Command -Session $Session -ScriptBlock {
            param($csPath, $resultPath, $csContent, $targetContains, $newName)

            $ErrorActionPreference = 'Stop'
            try {
                [System.IO.File]::WriteAllText($csPath, $csContent,
                    [System.Text.Encoding]::UTF8)

                $src = [System.IO.File]::ReadAllText($csPath)
                Add-Type -TypeDefinition $src -Language CSharp -ErrorAction Stop

                $result = [EndpointRenameApi]::RenameFirstMatching(
                    $targetContains, $newName)

                [System.IO.File]::WriteAllText($resultPath, $result,
                    [System.Text.Encoding]::UTF8)
                $result
            }
            catch {
                $errMsg = "ERROR: $($_.Exception.Message)"
                [System.IO.File]::WriteAllText($resultPath, $errMsg,
                    [System.Text.Encoding]::UTF8)
                throw
            }
        } -ArgumentList `
            $guestCsPath, `
            $guestResultPath, `
            $csCode, `
            $TargetContains, `
            $NewName

        $renameResult = ([string]$renameResult).Trim()

        if ($renameResult -match '^ERROR:') {
            throw "Rename failed: $renameResult"
        }

        return $renameResult
    }
}

Describe "PKEY: rename audio endpoint via IPropertyStore" {
    BeforeAll {
        $script:Session = Reset-Vm @VmContext
    }

    AfterAll {
        if ($script:Session) {
            Remove-PSSession $script:Session -ErrorAction SilentlyContinue
        }
    }

    It "changes FriendlyName and the new name is visible in AudioEndpoint PnP devices" {
        # $newName is used as the PKEY_Device_DeviceDesc value (pid=2).
        # After the write the computed FriendlyName (pid=14) becomes
        # "$newName (Cable Virtual Audio Device)", so PnP reports that composite.
        $targetContains = "Cable Virtual Audio Device"
        $newName        = "PKEY Rename Test"

        $renameResult = Invoke-PkeyRename -Session $script:Session `
            -TargetContains $targetContains -NewName $newName

        # The result string contains "After='<compositeFullName>'".
        # -Match is a regex contains-check, so "$newName" as a substring passes.
        $renameResult | Should -Match "After='$newName"

        # pnputil rescan and wait a moment for the endpoint list to refresh.
        Invoke-Command -Session $script:Session -ScriptBlock {
            pnputil /scan-devices | Out-Null
        }
        Start-Sleep -Seconds 5

        $afterNames = Invoke-Command -Session $script:Session -ScriptBlock {
            Get-PnpDevice -Class AudioEndpoint -ErrorAction SilentlyContinue |
                Select-Object -ExpandProperty FriendlyName
        }

        # The full PnP FriendlyName is "$newName (Cable Virtual Audio Device)".
        # Verify at least one endpoint name starts with $newName.
        $matchedName = $afterNames | Where-Object { $_ -like "$newName*" } |
            Select-Object -First 1
        $matchedName | Should -Not -BeNullOrEmpty
    }
}
