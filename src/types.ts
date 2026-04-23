import { Edge, NodeTypes } from "@xyflow/react";

import AudioInputDevice, { AudioInputDeviceNode } from "./nodes/AudioInputDevice";
import AudioOutputDevice, { AudioOutputDeviceNode } from "./nodes/AudioOutputDevice";
import VirtualAudioInput, { VirtualAudioInputNode } from "./nodes/VirtualAudioInput";
import VirtualAudioOutput, { VirtualAudioOutputNode } from "./nodes/VirtualAudioOutput";
import SpectrumAnalyzer, { SpectrumAnalyzerNode } from "./nodes/SpectrumAnalyzer";
import WaveformMonitor, { WaveformMonitorNode } from "./nodes/WaveformMonitor";
import AppAudioCapture, { AppAudioCaptureNode } from "./nodes/AppAudioCapture";
import Mixer, { MixerNodeType } from "./nodes/Mixer";

export interface WindowInfo {
  processId: number;
  title: string;
}

export interface AudioDevice {
  id: string;
  readableName: string;
  descriptions: string[] | null;

  frequency: number;
  channels: number;
  bitsPerSample: number;
}

/** A virtual audio device created in the CableAudio driver. */
export interface VirtualDevice {
  /** Hex-encoded 16-byte device ID from the driver. */
  id: string;
  /** User-chosen friendly name. */
  name: string;
  /** "render" or "capture". */
  deviceType: string;
}

export const nodeTypes = {
  audioInputDevice: AudioInputDevice,
  audioOutputDevice: AudioOutputDevice,
  virtualAudioInput: VirtualAudioInput,
  virtualAudioOutput: VirtualAudioOutput,
  spectrumAnalyzer: SpectrumAnalyzer,
  waveformMonitor: WaveformMonitor,
  appAudioCapture: AppAudioCapture,
  mixer: Mixer,
} satisfies NodeTypes;

export type NodeType =
  | AudioInputDeviceNode
  | AudioOutputDeviceNode
  | VirtualAudioInputNode
  | VirtualAudioOutputNode
  | SpectrumAnalyzerNode
  | WaveformMonitorNode
  | AppAudioCaptureNode
  | MixerNodeType;

export type EdgeType = Edge<AudioEdge>;

export interface AudioGraph {
  nodes: AudioNode[];
  edges: AudioEdge[];
}

export type AudioNode = {
  type:
    | "audioInputDevice"
    | "audioOutputDevice"
    | "virtualAudioInput"
    | "virtualAudioOutput"
    | "spectrumAnalyzer"
    | "waveformMonitor"
    | "appAudioCapture"
    | "mixer";
  data: { device: AudioDevice | null; id: string } | { deviceId: string; name: string; id: string } | { fftSize: number; id: string } | { windowSize: number; id: string } | { processId: number; windowTitle: string; id: string } | { id: string };
};

export type AudioEdge = {
  id: string;
  from: string;
  to: string;
  /** Target handle ID (e.g. "input-a", "input-b" for Mixer node) */
  toHandle?: string;

  frequency?: number;
  channels?: number;
  bitsPerSample?: number;
};

/** Serialized graph file saved to / loaded from disk. */
export interface CableGraphFile {
  version: 1;
  nodes: NodeType[];
  edges: EdgeType[];
}

/** Per-frame render data returned by `get_node_render_data` for visualizer nodes. */
export type NodeRenderData =
  | { type: "spectrumAnalyzer"; data: { bins: number[] } }
  | { type: "waveformMonitor"; data: { samples: number[] } };
