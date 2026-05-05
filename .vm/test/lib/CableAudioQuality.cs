// Audio quality analysis library for CableAudio VM integration tests.
// Renders a 440 Hz sine wave through the Cable routing pipeline for 5 seconds,
// captures the result, and measures silence gaps as a proxy for audio dropouts.
//
// Used by: AudioQualityHardening.Tests.ps1

using System;
using System.Collections.Generic;
using System.IO;
using System.Runtime.InteropServices;
using System.Threading;

public static class AudioQualityProbe
{
    const float SIGNAL_FREQ_HZ = 440f;
    const float SIGNAL_AMP     = 0.25f;   // -12 dBFS

    // 500 loops x 10 ms = 5 s of render+capture
    const int  RECORD_LOOPS      = 500;
    const uint FRAMES_PER_LOOP   = 480;   // 10 ms at 48 kHz
    const long HNS_BUFFER        = 2000000; // 200 ms in 100-ns units

    // RMS below this threshold (linear) counts as silence (-60 dBFS)
    const double SILENCE_THRESHOLD = 0.001;

    // Analysis window: 10 ms = 480 frames at 48 kHz
    const int WINDOW_FRAMES = 480;

    // Skip first and last 500 ms to avoid pipeline startup / teardown transients
    const int SKIP_FRAMES = 24000; // 0.5 s at 48 kHz

    // -----------------------------------------------------------------------

    static float SineSample(long index, int sampleRate)
    {
        return SIGNAL_AMP * (float)Math.Sin(2.0 * Math.PI * SIGNAL_FREQ_HZ * index / sampleRate);
    }

    static void FillRenderBuffer(IAudioRenderClient renderSvc, uint frames,
                                  WAVEFORMATEX fmt, ref long renderPos)
    {
        IntPtr pData;
        CableWasapi.ThrowIfFailed(
            renderSvc.GetBuffer(frames, out pData), "FillRenderBuffer::GetBuffer");

        int blockAlign    = fmt.nBlockAlign;
        int bytesPerSample = fmt.wBitsPerSample / 8;
        bool isFloat32    = (fmt.wBitsPerSample == 32);
        bool isInt16      = (fmt.wBitsPerSample == 16);
        int  byteCount    = (int)(frames * (uint)blockAlign);

        byte[] buf = new byte[byteCount];

        for (int f = 0; f < (int)frames; f++)
        {
            float v = SineSample(renderPos + f, (int)fmt.nSamplesPerSec);
            for (int ch = 0; ch < fmt.nChannels; ch++)
            {
                int off = f * blockAlign + ch * bytesPerSample;
                if (isFloat32)
                {
                    byte[] fb = BitConverter.GetBytes(v);
                    Array.Copy(fb, 0, buf, off, 4);
                }
                else if (isInt16)
                {
                    short s = (short)Math.Max(short.MinValue,
                        Math.Min(short.MaxValue, (int)(v * 32767f)));
                    buf[off]     = (byte)(s & 0xFF);
                    buf[off + 1] = (byte)((s >> 8) & 0xFF);
                }
                else // int32
                {
                    int s = (int)(v * 2147483647f);
                    byte[] ib = BitConverter.GetBytes(s);
                    Array.Copy(ib, 0, buf, off, 4);
                }
            }
        }

        Marshal.Copy(buf, 0, pData, byteCount);
        CableWasapi.ThrowIfFailed(
            renderSvc.ReleaseBuffer(frames, 0), "FillRenderBuffer::ReleaseBuffer");

        renderPos += (long)frames;
    }

    static void DrainCapture(IAudioCaptureClient captureSvc, WAVEFORMATEX fmt,
                              List<float> captured)
    {
        uint next;
        CableWasapi.ThrowIfFailed(
            captureSvc.GetNextPacketSize(out next), "DrainCapture::GetNextPacketSize");

        int  blockAlign    = fmt.nBlockAlign;
        int  bytesPerSample = fmt.wBitsPerSample / 8;
        bool isFloat32     = (fmt.wBitsPerSample == 32);
        bool isInt16       = (fmt.wBitsPerSample == 16);

        while (next > 0)
        {
            IntPtr pData;
            uint   frames, flags;
            ulong  pos, qpc;
            CableWasapi.ThrowIfFailed(
                captureSvc.GetBuffer(out pData, out frames, out flags, out pos, out qpc),
                "DrainCapture::GetBuffer");

            int byteCount = (int)(frames * (uint)blockAlign);
            if (byteCount > 0)
            {
                byte[] buf = new byte[byteCount];
                Marshal.Copy(pData, buf, 0, byteCount);

                for (int f = 0; f < (int)frames; f++)
                {
                    for (int ch = 0; ch < fmt.nChannels; ch++)
                    {
                        int   off = f * blockAlign + ch * bytesPerSample;
                        float v;
                        if (isFloat32)
                            v = BitConverter.ToSingle(buf, off);
                        else if (isInt16)
                            v = (short)(buf[off] | (buf[off + 1] << 8)) / 32768f;
                        else
                            v = BitConverter.ToInt32(buf, off) / 2147483648f;

                        captured.Add(v);
                    }
                }
            }

            CableWasapi.ThrowIfFailed(
                captureSvc.ReleaseBuffer(frames), "DrainCapture::ReleaseBuffer");
            CableWasapi.ThrowIfFailed(
                captureSvc.GetNextPacketSize(out next), "DrainCapture::GetNextPacketSize(loop)");
        }
    }

    static void WriteWav16Pcm(string path, List<float> samples, int sampleRate, int channels)
    {
        int dataBytes = samples.Count * 2; // 16-bit = 2 bytes per sample
        using (var fs = new FileStream(path, FileMode.Create, FileAccess.Write))
        using (var bw = new BinaryWriter(fs))
        {
            bw.Write(new byte[] { (byte)'R', (byte)'I', (byte)'F', (byte)'F' });
            bw.Write(36 + dataBytes);
            bw.Write(new byte[] { (byte)'W', (byte)'A', (byte)'V', (byte)'E' });
            bw.Write(new byte[] { (byte)'f', (byte)'m', (byte)'t', (byte)' ' });
            bw.Write(16);                        // fmt chunk size
            bw.Write((short)1);                  // PCM
            bw.Write((short)channels);
            bw.Write(sampleRate);
            bw.Write(sampleRate * channels * 2); // byte rate
            bw.Write((short)(channels * 2));     // block align
            bw.Write((short)16);                 // bits per sample
            bw.Write(new byte[] { (byte)'d', (byte)'a', (byte)'t', (byte)'a' });
            bw.Write(dataBytes);
            foreach (float s in samples)
            {
                short v = (short)Math.Max(short.MinValue,
                    Math.Min(short.MaxValue, (int)(s * 32767f)));
                bw.Write(v);
            }
        }
    }

    // Returns the longest consecutive silence window in milliseconds.
    // A silence window is a 10 ms block whose per-sample RMS is below SILENCE_THRESHOLD.
    static double MaxSilenceDurationMs(List<float> samples, int channels, int sampleRate)
    {
        int    windowSamples = (sampleRate / 100) * channels; // 10 ms worth of interleaved samples
        double maxSilenceMs  = 0;
        double curSilenceMs  = 0;
        int    count         = samples.Count;

        for (int i = 0; i + windowSamples <= count; i += windowSamples)
        {
            double sumSq = 0;
            for (int j = i; j < i + windowSamples; j++)
                sumSq += (double)samples[j] * samples[j];

            double rms = Math.Sqrt(sumSq / windowSamples);

            if (rms < SILENCE_THRESHOLD)
            {
                curSilenceMs += 10.0;
                if (curSilenceMs > maxSilenceMs)
                    maxSilenceMs = curSilenceMs;
            }
            else
            {
                curSilenceMs = 0;
            }
        }

        return maxSilenceMs;
    }

    // Render a 440 Hz sine wave into the render endpoint, capture from the
    // capture endpoint (both must already be routed by the Cable runtime),
    // and analyse the captured PCM for silence gaps.
    //
    // Returns: "QUALITY: maxSilenceMs=X capturedFrames=Y wavPath=..."
    public static string Run(string renderEndpointId, string captureEndpointId)
    {
        var enumerator = (IMMDeviceEnumerator)Activator.CreateInstance(
            Type.GetTypeFromCLSID(CableWasapi.CLSID_MMDeviceEnumerator));

        IMMDevice renderDev, captureDev;
        CableWasapi.ThrowIfFailed(
            enumerator.GetDevice(renderEndpointId,  out renderDev),  "GetDevice(render)");
        CableWasapi.ThrowIfFailed(
            enumerator.GetDevice(captureEndpointId, out captureDev), "GetDevice(capture)");

        var renderAc  = CableWasapi.ActivateAudioClient(renderDev);
        var captureAc = CableWasapi.ActivateAudioClient(captureDev);

        IntPtr pRenderFmt  = CableWasapi.GetMixFormatPtr(renderAc);
        IntPtr pCaptureFmt = CableWasapi.GetMixFormatPtr(captureAc);
        WAVEFORMATEX renderFmt  = (WAVEFORMATEX)Marshal.PtrToStructure(pRenderFmt,  typeof(WAVEFORMATEX));
        WAVEFORMATEX captureFmt = (WAVEFORMATEX)Marshal.PtrToStructure(pCaptureFmt, typeof(WAVEFORMATEX));

        try
        {
            CableWasapi.ThrowIfFailed(
                renderAc.Initialize(CableWasapi.AUDCLNT_SHAREMODE_SHARED, 0, HNS_BUFFER, 0, pRenderFmt,  IntPtr.Zero),
                "Render Initialize");
            CableWasapi.ThrowIfFailed(
                captureAc.Initialize(CableWasapi.AUDCLNT_SHAREMODE_SHARED, 0, HNS_BUFFER, 0, pCaptureFmt, IntPtr.Zero),
                "Capture Initialize");

            uint renderBufFrames;
            CableWasapi.ThrowIfFailed(
                renderAc.GetBufferSize(out renderBufFrames), "Render GetBufferSize");

            IntPtr pRenderSvc, pCaptureSvc;
            CableWasapi.ThrowIfFailed(
                renderAc.GetService( ref CableWasapi.IID_IAudioRenderClient,  out pRenderSvc),  "Render GetService");
            CableWasapi.ThrowIfFailed(
                captureAc.GetService(ref CableWasapi.IID_IAudioCaptureClient, out pCaptureSvc), "Capture GetService");

            var renderSvc  = (IAudioRenderClient) Marshal.GetObjectForIUnknown(pRenderSvc);
            var captureSvc = (IAudioCaptureClient)Marshal.GetObjectForIUnknown(pCaptureSvc);

            // Pre-fill the entire render buffer before starting both streams.
            long renderPos = 0;
            FillRenderBuffer(renderSvc, renderBufFrames, renderFmt, ref renderPos);

            CableWasapi.ThrowIfFailed(captureAc.Start(), "Capture Start");
            CableWasapi.ThrowIfFailed(renderAc.Start(),  "Render Start");

            var captured = new List<float>(
                RECORD_LOOPS * (int)FRAMES_PER_LOOP * captureFmt.nChannels);

            for (int i = 0; i < RECORD_LOOPS; i++)
            {
                Thread.Sleep(10);

                uint padding;
                renderAc.GetCurrentPadding(out padding);
                uint avail = renderBufFrames - padding;
                if (avail >= FRAMES_PER_LOOP)
                    FillRenderBuffer(renderSvc, FRAMES_PER_LOOP, renderFmt, ref renderPos);

                DrainCapture(captureSvc, captureFmt, captured);
            }

            // Allow pipeline tail to drain before stopping.
            Thread.Sleep(200);
            DrainCapture(captureSvc, captureFmt, captured);

            renderAc.Stop();
            captureAc.Stop();

            // Trim startup and teardown transients before running silence analysis.
            int skipSamples  = SKIP_FRAMES * captureFmt.nChannels;
            int totalSamples = captured.Count;
            int sliceLen     = Math.Max(0, totalSamples - 2 * skipSamples);
            var analysisSlice = captured.GetRange(skipSamples, sliceLen);

            // Write the trimmed slice to WAV so ffmpeg sees the same window as the C# analysis.
            string wavPath = @"C:\CableAudio\quality-probe.wav";
            WriteWav16Pcm(wavPath, analysisSlice, (int)captureFmt.nSamplesPerSec, captureFmt.nChannels);

            double maxSilenceMs = MaxSilenceDurationMs(
                analysisSlice, captureFmt.nChannels, (int)captureFmt.nSamplesPerSec);

            return "QUALITY: maxSilenceMs=" + maxSilenceMs.ToString("F1")
                 + " capturedFrames=" + (totalSamples / captureFmt.nChannels)
                 + " wavPath=" + wavPath;
        }
        finally
        {
            CableWasapi.CoTaskMemFree(pRenderFmt);
            CableWasapi.CoTaskMemFree(pCaptureFmt);
        }
    }
}
