import { describe, expect, it } from "vitest";

import { audioType, frequencyType, NONE } from "@/graph/edge-type";
import audioInputDeviceDef from "@/nodes/AudioInputDevice";
import audioOutputDeviceDef from "@/nodes/AudioOutputDevice";
import channelMergeDef from "@/nodes/ChannelMerge";
import channelSplitDef from "@/nodes/ChannelSplit";
import gainDef from "@/nodes/Gain";
import mixerDef from "@/nodes/Mixer";
import spectrumAnalyzerDef from "@/nodes/SpectrumAnalyzer";
import virtualAudioInputDef from "@/nodes/VirtualAudioInput";
import virtualAudioOutputDef from "@/nodes/VirtualAudioOutput";
import waveformMonitorDef from "@/nodes/WaveformMonitor";

const dev48k2c16 = {
  id: "dev",
  readableName: "Dev",
  descriptions: null,
  frequency: 48000,
  channels: 2,
  bitsPerSample: 16,
};

describe("per-node validators", () => {
  it("AudioInputDevice produces audio type derived from device", () => {
    const r = audioInputDeviceDef.validate!({ device: dev48k2c16 } as any, {});
    expect(r.ok).toBe(true);
    expect(r.producedOutputs["AudioInputDevice-source"]).toEqual(audioType(2, 48000, 16));
  });

  it("AudioInputDevice without device produces none", () => {
    const r = audioInputDeviceDef.validate!({ device: null } as any, {});
    expect(r.ok).toBe(true);
    expect(r.producedOutputs["AudioInputDevice-source"]).toEqual(NONE);
  });

  it("AudioOutputDevice ok=true when input matches device format", () => {
    const r = audioOutputDeviceDef.validate!(
      { device: dev48k2c16 } as any,
      { "AudioOutputDevice-target": audioType(2, 48000, 16) },
    );
    expect(r.ok).toBe(true);
  });

  it("AudioOutputDevice ok=false when input mismatches device format", () => {
    const r = audioOutputDeviceDef.validate!(
      { device: dev48k2c16 } as any,
      { "AudioOutputDevice-target": audioType(2, 44100, 16) },
    );
    expect(r.ok).toBe(false);
  });

  it("Gain passes the input audio type through", () => {
    const r = gainDef.validate!(
      { gain: 1 } as any,
      { "Gain-target": audioType(1, 48000, 24) },
    );
    expect(r.ok).toBe(true);
    expect(r.producedOutputs["Gain-source"]).toEqual(audioType(1, 48000, 24));
  });

  it("Mixer flags ok=false when its two inputs disagree", () => {
    const r = mixerDef.validate!(
      {} as any,
      {
        "input-a": audioType(2, 48000, 16),
        "input-b": audioType(2, 44100, 16),
      },
    );
    expect(r.ok).toBe(false);
  });

  it("Mixer ok=true when one input is none (passthrough)", () => {
    const r = mixerDef.validate!(
      {} as any,
      { "input-a": audioType(2, 48000, 16), "input-b": NONE },
    );
    expect(r.ok).toBe(true);
  });

  it("ChannelSplit fans out a stereo input into two mono outputs", () => {
    const r = channelSplitDef.validate!(
      {} as any,
      { "ChannelSplit-target": audioType(2, 48000, 16) },
    );
    expect(r.ok).toBe(true);
    expect(r.producedOutputs["ch-0"]).toEqual(audioType(1, 48000, 16));
    expect(r.producedOutputs["ch-1"]).toEqual(audioType(1, 48000, 16));
  });

  it("ChannelMerge merges 2 mono inputs into stereo output", () => {
    const r = channelMergeDef.validate!(
      { inputCount: 2 } as any,
      {
        "ch-0": audioType(1, 48000, 16),
        "ch-1": audioType(1, 48000, 16),
      },
    );
    expect(r.ok).toBe(true);
    expect(r.producedOutputs["ChannelMerge-source"]).toEqual(audioType(2, 48000, 16));
  });

  it("ChannelMerge ok=false when inputs have mismatched format", () => {
    const r = channelMergeDef.validate!(
      { inputCount: 2 } as any,
      {
        "ch-0": audioType(1, 48000, 16),
        "ch-1": audioType(1, 44100, 16),
      },
    );
    expect(r.ok).toBe(false);
  });

  it("ChannelMerge ok=false when inputs are not mono", () => {
    const r = channelMergeDef.validate!(
      { inputCount: 2 } as any,
      {
        "ch-0": audioType(2, 48000, 16),
        "ch-1": audioType(1, 48000, 16),
      },
    );
    expect(r.ok).toBe(false);
  });

  it("ChannelMerge produces none when all inputs are none", () => {
    const r = channelMergeDef.validate!(
      { inputCount: 2 } as any,
      { "ch-0": NONE, "ch-1": NONE },
    );
    expect(r.ok).toBe(true);
    expect(r.producedOutputs["ChannelMerge-source"]).toEqual(NONE);
  });

  it("ChannelMerge merges 4 mono inputs into 4ch output", () => {
    const inputs = Object.fromEntries(
      [0, 1, 2, 3].map((i) => [`ch-${i}`, audioType(1, 48000, 32)]),
    );
    const r = channelMergeDef.validate!({ inputCount: 4 } as any, inputs);
    expect(r.ok).toBe(true);
    expect(r.producedOutputs["ChannelMerge-source"]).toEqual(audioType(4, 48000, 32));
  });


  it("SpectrumAnalyzer passes audio through (FFT output is rendered, not edge-carried)", () => {
    const r = spectrumAnalyzerDef.validate!(
      { fftSize: 1024 } as any,
      { "SpectrumAnalyzer-target": audioType(2, 48000, 16) },
    );
    expect(r.ok).toBe(true);
    expect(r.producedOutputs["SpectrumAnalyzer-source"]).toEqual(audioType(2, 48000, 16));
  });

  it("WaveformMonitor passes audio through", () => {
    const r = waveformMonitorDef.validate!(
      {} as any,
      { "WaveformMonitor-target": audioType(2, 48000, 24) },
    );
    expect(r.ok).toBe(true);
    expect(r.producedOutputs["WaveformMonitor-source"]).toEqual(audioType(2, 48000, 24));
  });

  it("VirtualAudioInput produces default audio when deviceId is set", () => {
    const r = virtualAudioInputDef.validate!(
      { deviceId: "abc", name: "Mic" } as any,
      {},
    );
    expect(r.ok).toBe(true);
    expect(r.producedOutputs["VirtualAudioInput-source"]?.kind).toBe("audio");
  });

  it("VirtualAudioOutput accepts the default audio format", () => {
    const r = virtualAudioOutputDef.validate!(
      { deviceId: "abc", name: "Speaker" } as any,
      { "VirtualAudioOutput-target": audioType(2, 48000, 32) },
    );
    expect(r.ok).toBe(true);
  });
});

describe("frequency edge equality", () => {
  it("frequency types with different bins are not equal", () => {
    const a = frequencyType(2, 48000, 1024);
    const b = frequencyType(2, 48000, 2048);
    expect(a).not.toEqual(b);
  });
});
