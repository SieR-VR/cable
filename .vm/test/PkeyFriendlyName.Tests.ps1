# PKEY FriendlyName test for CableAudio driver.
# Device creation and removal are handled via the headless REST API.
# The rename itself still uses IPropertyStore::SetValue (WASAPI COM) because
# that code path is what the production app exercises.
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

BeforeAll {
    . (Join-Path $PSScriptRoot "common.ps1")
    $script:wasapiCs = Get-CSharpLib "CableWasapi"

    # Rename a WASAPI endpoint by its ID using IPropertyStore.
    # Accepts the endpoint ID returned by the REST API, so no IOCTL or
    # polling-by-name is required here.
    $script:renameCs = @'
public static class EndpointRename
{
    public static string Run(string endpointId, string newName)
    {
        var enumerator = (IMMDeviceEnumerator)Activator.CreateInstance(
            Type.GetTypeFromCLSID(CableWasapi.CLSID_MMDeviceEnumerator));

        IMMDevice target;
        int hr = enumerator.GetDevice(endpointId, out target);
        if (hr < 0 || target == null)
            return "ENDPOINT_NOT_FOUND: hr=0x" + ((uint)hr).ToString("X") + " id=" + endpointId;

        string original = CableWasapi.ReadFriendlyName(target);

        IPropertyStore storeRW;
        CableWasapi.ThrowIfFailed(
            target.OpenPropertyStore(CableWasapi.STGM_READWRITE, out storeRW),
            "OpenPropertyStore(readwrite)");

        PROPVARIANT pv  = CableWasapi.MakeStringPropVariant(newName);
        PROPERTYKEY key = CableWasapi.PkeyDeviceDesc();
        try
        {
            CableWasapi.ThrowIfFailed(storeRW.SetValue(ref key, ref pv), "SetValue(DeviceDesc)");
            CableWasapi.ThrowIfFailed(storeRW.Commit(), "Commit");
        }
        finally
        {
            if (pv.pointerValue != System.IntPtr.Zero)
                System.Runtime.InteropServices.Marshal.FreeCoTaskMem(pv.pointerValue);
        }

        IPropertyStore storeRead;
        CableWasapi.ThrowIfFailed(
            target.OpenPropertyStore(CableWasapi.STGM_READ, out storeRead),
            "OpenPropertyStore(read-after)");
        string after = CableWasapi.ReadStringProp(storeRead, CableWasapi.PkeyFriendlyName());

        return "Before='" + original + "' | After='" + after + "'";
    }
}
'@
}

Describe "PKEY: rename audio endpoint via IPropertyStore" {
    BeforeAll {
        $script:Session = Reset-Vm @VmContext
        Copy-GuestAppExe -Session $script:Session -ExePath $VmContext.AppExePath -ReuseVm $VmContext.ReuseVm
        Start-GuestHeadlessApp -Session $script:Session
        Invoke-AppRpc -Session $script:Session -Method POST -Path "/driver/connect" | Out-Null
    }

    AfterAll {
        Stop-GuestHeadlessApp -Session $script:Session
        if ($script:Session) {
            Remove-PSSession $script:Session -ErrorAction SilentlyContinue
        }
    }

    It "changes FriendlyName and the new name is visible in AudioEndpoint PnP devices" {
        $newName = "PKEY Rename Test"

        $device = Invoke-AppRpc -Session $script:Session -Method POST -Path "/virtual-devices" `
            -Body @{ name = "PkeyTestSpeaker"; deviceType = "render" }

        $device.id        | Should -Not -BeNullOrEmpty
        $device.endpointId | Should -Not -BeNullOrEmpty

        try {
            $endpointId = $device.endpointId

            $renameResult = Invoke-GuestCSharpTest -Session $script:Session `
                -CSharpSources @($script:wasapiCs, $script:renameCs) `
                -Script ([scriptblock]::Create("[EndpointRename]::Run('$endpointId', '$newName')")) `
                -TempFileName "pkey-rename"

            $renameResult = ([string]$renameResult).Trim()
            $renameResult | Should -Match "After='$newName"

            Invoke-Command -Session $script:Session -ScriptBlock {
                pnputil /scan-devices | Out-Null
            }

            $matchedName = $null
            $deadline = (Get-Date).AddSeconds(10)
            while ((Get-Date) -lt $deadline) {
                Start-Sleep -Seconds 1
                $afterNames = Invoke-Command -Session $script:Session -ScriptBlock {
                    Get-PnpDevice -Class AudioEndpoint -ErrorAction SilentlyContinue |
                        Select-Object -ExpandProperty FriendlyName
                }
                $matchedName = $afterNames | Where-Object { $_ -like "$newName*" } |
                    Select-Object -First 1
                if ($matchedName) { break }
            }
            $matchedName | Should -Not -BeNullOrEmpty
        }
        finally {
            Invoke-AppRpc -Session $script:Session -Method DELETE -Path "/virtual-devices/$($device.id)" | Out-Null
        }
    }
}
