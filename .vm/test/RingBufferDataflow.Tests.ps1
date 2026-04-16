# Ring buffer data flow test for CableAudio driver.
# Verifies that audio played to a dynamic virtual render device flows into the
# mapped ring buffer (write_index advances, data is non-zero).
#
# This test does NOT require cable-tauri.exe — it uses raw IOCTLs + WASAPI to
# prove the driver-side pipeline (DMA -> ring buffer) is working.

BeforeAll {
    . (Join-Path $PSScriptRoot "common.ps1")
    $script:ioctlCs = Get-CSharpLib "CableIoctl"
    $script:wasapiCs = Get-CSharpLib "CableWasapi"

    # Test-specific C# class: orchestrates IOCTL device creation, ring buffer
    # mapping, WASAPI rendering, and ring buffer header verification.
    $script:dataFlowCs = @'
public static class RingBufferDataFlowTest
{
    const uint MAGIC_CBRB = 0x42524243u;

    public static string Run()
    {
        var log = new System.Collections.Generic.List<string>();

        // Step 1: Create a render device via IOCTL
        string createResult = CableIoctl.Create(0, "DataFlowTestDevice");
        log.Add(createResult);
        if (!createResult.StartsWith("CREATE OK")) return string.Join("\n", log);

        string deviceId = CableIoctl.ParseCreateId(createResult);

        // Step 2: Map the ring buffer
        string mapResult = CableIoctl.MapRingBuffer(deviceId);
        log.Add(mapResult);
        if (!mapResult.StartsWith("MAP OK"))
        {
            CableIoctl.Remove(deviceId);
            return string.Join("\n", log);
        }

        ulong mapAddr = CableIoctl.ParseMapAddress(mapResult);
        uint dataSize = CableIoctl.ParseMapDataSize(mapResult);

        // Step 3: Read ring buffer header (initial state)
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

        // Step 4: Wait for endpoint to appear, then render audio
        string deviceName = "Cable Virtual Audio";
        IMMDevice renderDevice = null;
        for (int attempt = 0; attempt < 40; attempt++)
        {
            Thread.Sleep(500);
            try { renderDevice = CableWasapi.FindDeviceByName(0, deviceName); }
            catch { }
            if (renderDevice != null) break;
        }

        if (renderDevice == null)
        {
            log.Add("ENDPOINT_NOT_FOUND: no render endpoint with name containing '" + deviceName + "' after 20s");
            log.Add(CableWasapi.ListAllDevices(0));
            goto cleanup;
        }

        log.Add("ENDPOINT_FOUND: " + CableWasapi.ReadFriendlyName(renderDevice));

        {
            var audioClient = CableWasapi.ActivateAudioClient(renderDevice);

            IntPtr pFormat = CableWasapi.GetMixFormatPtr(audioClient);
            WAVEFORMATEX fmt = (WAVEFORMATEX)Marshal.PtrToStructure(pFormat, typeof(WAVEFORMATEX));

            log.Add("FORMAT: tag=" + fmt.wFormatTag + ",ch=" + fmt.nChannels +
                ",sr=" + fmt.nSamplesPerSec + ",bits=" + fmt.wBitsPerSample +
                ",align=" + fmt.nBlockAlign);

            long hnsBuffer = 2000000; // 200ms
            CableWasapi.ThrowIfFailed(audioClient.Initialize(0, 0, hnsBuffer, 0, pFormat, IntPtr.Zero),
                "IAudioClient::Initialize");

            uint bufferFrames;
            CableWasapi.ThrowIfFailed(audioClient.GetBufferSize(out bufferFrames), "GetBufferSize");

            IntPtr pRenderService;
            CableWasapi.ThrowIfFailed(audioClient.GetService(ref CableWasapi.IID_IAudioRenderClient, out pRenderService),
                "GetService(IAudioRenderClient)");
            var renderClient = (IAudioRenderClient)Marshal.GetObjectForIUnknown(pRenderService);

            CableWasapi.ThrowIfFailed(audioClient.Start(), "IAudioClient::Start");

            long writeIdxBefore = Marshal.ReadInt64(new IntPtr((long)mapAddr), 0);
            log.Add("WRITE_IDX_BEFORE_RENDER: " + writeIdxBefore);

            // Pump several buffers of non-zero audio data
            for (int i = 0; i < 20; i++)
            {
                uint padding;
                CableWasapi.ThrowIfFailed(audioClient.GetCurrentPadding(out padding), "GetCurrentPadding");
                uint available = bufferFrames - padding;
                if (available > 0)
                {
                    uint toWrite = Math.Min(available, 480);
                    IntPtr pData;
                    CableWasapi.ThrowIfFailed(renderClient.GetBuffer(toWrite, out pData), "GetBuffer");

                    int bytes = checked((int)(toWrite * fmt.nBlockAlign));
                    byte[] audio = new byte[bytes];

                    for (int b = 0; b < bytes; b++)
                    {
                        int frameIdx = b / fmt.nBlockAlign;
                        audio[b] = (byte)((frameIdx % 48 < 24) ? 0x40 : 0xC0);
                    }

                    Marshal.Copy(audio, 0, pData, bytes);
                    CableWasapi.ThrowIfFailed(renderClient.ReleaseBuffer(toWrite, 0), "ReleaseBuffer");
                }
                Thread.Sleep(25);
            }

            Thread.Sleep(200);

            long writeIdxAfter = Marshal.ReadInt64(new IntPtr((long)mapAddr), 0);
            log.Add("WRITE_IDX_AFTER_RENDER: " + writeIdxAfter);

            long bytesWritten = writeIdxAfter - writeIdxBefore;
            log.Add("BYTES_WRITTEN_TO_RINGBUF: " + bytesWritten);

            if (bytesWritten > 0)
                log.Add("DATAFLOW_OK: driver wrote " + bytesWritten + " bytes to ring buffer");
            else
                log.Add("DATAFLOW_FAIL: write_index did not advance (no data in ring buffer)");

            // Check if any non-zero data in the ring buffer data region
            int nonZeroCount = 0;
            {
                IntPtr dataStart = new IntPtr((long)mapAddr + 40);
                int checkLen = (int)dataSize;
                for (int i = 0; i < checkLen; i++)
                {
                    if (Marshal.ReadByte(dataStart, i) != 0) nonZeroCount++;
                }
            }
            log.Add("NON_ZERO_DATA_BYTES: " + nonZeroCount + " (checked all " + (int)dataSize + " bytes)");

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

            if (renderClient != null) Marshal.ReleaseComObject(renderClient);
            if (audioClient != null) Marshal.ReleaseComObject(audioClient);
            if (renderDevice != null) Marshal.ReleaseComObject(renderDevice);

            CableWasapi.CoTaskMemFree(pFormat);
        }

        GC.Collect();
        GC.WaitForPendingFinalizers();
        Thread.Sleep(500);

    cleanup:

        // Step 5: Cleanup
        string unmapResult = CableIoctl.UnmapRingBuffer(deviceId, mapAddr);
        log.Add(unmapResult);

        string removeResult = CableIoctl.RemoveWithRetry(deviceId, 10, 1000);
        log.Add(removeResult);

        return string.Join("\n", log);
    }
}
'@
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
        $rawResult = Invoke-GuestCSharpTest -Session $script:Session `
            -CSharpSources @($script:ioctlCs, $script:wasapiCs, $script:dataFlowCs) `
            -Script {
                [RingBufferDataFlowTest]::Run()
            } `
            -TempFileName "ringbuffer-dataflow-test"

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
