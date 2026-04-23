import { Edge, NodeTypes } from "@xyflow/react";

import AudioInputDevice, { AudioInputDeviceNode, toAudioNode as serializeAudioInputDevice } from "./nodes/AudioInputDevice";
import AudioOutputDevice, { AudioOutputDeviceNode, toAudioNode as serializeAudioOutputDevice } from "./nodes/AudioOutputDevice";
import VirtualAudioInput, { VirtualAudioInputNode, toAudioNode as serializeVirtualAudioInput } from "./nodes/VirtualAudioInput";
import VirtualAudioOutput, { VirtualAudioOutputNode, toAudioNode as serializeVirtualAudioOutput } from "./nodes/VirtualAudioOutput";
import SpectrumAnalyzer, { SpectrumAnalyzerNode, toAudioNode as serializeSpectrumAnalyzer } from "./nodes/SpectrumAnalyzer";
import WaveformMonitor, { WaveformMonitorNode, toAudioNode as serializeWaveformMonitor } from "./nodes/WaveformMonitor";
import AppAudioCapture, { AppAudioCaptureNode, toAudioNode as serializeAppAudioCapture } from "./nodes/AppAudioCapture";
import Mixer, { MixerNodeType, toAudioNode as serializeMixer } from "./nodes/Mixer";

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

/** 각 노드 타입을 IPC용 AudioNode로 직렬화하는 함수 맵. */
const nodeSerializers = {
  audioInputDevice: serializeAudioInputDevice,
  audioOutputDevice: serializeAudioOutputDevice,
  virtualAudioInput: serializeVirtualAudioInput,
  virtualAudioOutput: serializeVirtualAudioOutput,
  spectrumAnalyzer: serializeSpectrumAnalyzer,
  waveformMonitor: serializeWaveformMonitor,
  appAudioCapture: serializeAppAudioCapture,
  mixer: serializeMixer,
} satisfies { [K in NodeType["type"]]: (node: Extract<NodeType, { type: K }>) => AudioNode };

export function serializeNode(node: NodeType): AudioNode {
  const serializer = nodeSerializers[node.type] as (node: NodeType) => AudioNode;
  return serializer(node);
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
