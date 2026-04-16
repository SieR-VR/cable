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
    $script:ioctlCs  = Get-CSharpLib "CableIoctl"
    $script:wasapiCs  = Get-CSharpLib "CableWasapi"

    # Test-specific C# that uses both CableIoctl (to create the device) and
    # CableWasapi (to rename the endpoint via IPropertyStore).
    $script:renameCs = @'
public static class EndpointRename
{
    public static string Run(string newName)
    {
        // Create a render device so the endpoint appears
        string createResult = CableIoctl.Create(0, "PkeyTestSpeaker");
        if (!createResult.StartsWith("CREATE OK"))
            return "SETUP_FAIL: " + createResult;
        string deviceId = CableIoctl.ParseCreateId(createResult);

        try
        {
            // Wait for WASAPI endpoint to appear
            IMMDevice target = null;
            string contains = "Cable Virtual Audio";
            for (int attempt = 0; attempt < 40; attempt++)
            {
                Thread.Sleep(500);
                target = CableWasapi.FindDeviceByName(0, contains);
                if (target != null) break;
            }
            if (target == null)
                return "ENDPOINT_NOT_FOUND: " + CableWasapi.ListAllDevices(0);

            string original = CableWasapi.ReadFriendlyName(target);
            string endpointId;
            CableWasapi.ThrowIfFailed(target.GetId(out endpointId), "GetId");

            // Open RW store and write PKEY_Device_DeviceDesc (pid=2).
            IPropertyStore storeRW;
            CableWasapi.ThrowIfFailed(target.OpenPropertyStore(CableWasapi.STGM_READWRITE, out storeRW),
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

            // Re-read FriendlyName (pid=14) which is now recomputed.
            IPropertyStore storeRead;
            CableWasapi.ThrowIfFailed(target.OpenPropertyStore(CableWasapi.STGM_READ, out storeRead),
                          "OpenPropertyStore(read-after)");
            string after = CableWasapi.ReadStringProp(storeRead, CableWasapi.PkeyFriendlyName());

            return "EndpointId=" + endpointId +
                   " | Before='" + original  + "'" +
                   " | After='"  + after     + "'";
        }
        finally
        {
            CableIoctl.RemoveWithRetry(deviceId, 5, 500);
        }
    }
}
'@
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
        $newName = "PKEY Rename Test"

        $renameResult = Invoke-GuestCSharpTest -Session $script:Session `
            -CSharpSources @($script:ioctlCs, $script:wasapiCs, $script:renameCs) `
            -Script {
                $r = [EndpointRename]::Run("PKEY Rename Test")
                $r
            } `
            -TempFileName "pkey-rename"

        $renameResult = ([string]$renameResult).Trim()
        $renameResult | Should -Match "After='$newName"

        # pnputil rescan and poll for the renamed endpoint (up to 10s).
        Invoke-Command -Session $script:Session -ScriptBlock {
            pnputil /scan-devices | Out-Null
        }

        $afterNames = $null
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
}
