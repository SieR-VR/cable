import { Edge, NodeTypes } from "@xyflow/react";

import audioInputDeviceDef, { AudioInputDeviceNode } from "./nodes/AudioInputDevice";
import audioOutputDeviceDef, { AudioOutputDeviceNode } from "./nodes/AudioOutputDevice";
import virtualAudioInputDef, { VirtualAudioInputNode } from "./nodes/VirtualAudioInput";
import virtualAudioOutputDef, { VirtualAudioOutputNode } from "./nodes/VirtualAudioOutput";
import spectrumAnalyzerDef, { SpectrumAnalyzerNode } from "./nodes/SpectrumAnalyzer";
import waveformMonitorDef, { WaveformMonitorNode } from "./nodes/WaveformMonitor";
import appAudioCaptureDef, { AppAudioCaptureNode } from "./nodes/AppAudioCapture";
import mixerDef, { MixerNodeType } from "./nodes/Mixer";

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

const nodeDefs = {
  audioInputDevice: audioInputDeviceDef,
  audioOutputDevice: audioOutputDeviceDef,
  virtualAudioInput: virtualAudioInputDef,
  virtualAudioOutput: virtualAudioOutputDef,
  spectrumAnalyzer: spectrumAnalyzerDef,
  waveformMonitor: waveformMonitorDef,
  appAudioCapture: appAudioCaptureDef,
  mixer: mixerDef,
};

export const nodeTypes = Object.fromEntries(
  Object.entries(nodeDefs).map(([k, v]) => [k, v.component]),
) as NodeTypes;

export function serializeNode(node: NodeType): AudioNode {
  const def = nodeDefs[node.type as keyof typeof nodeDefs];
  return (def.toAudioNode as (n: NodeType) => AudioNode)(node);
}

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
