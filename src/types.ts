import { Edge, Node, NodeTypes } from "@xyflow/react";

import { NodeDefinition } from "./node-definition";
import appAudioCaptureDef from "./nodes/AppAudioCapture";
import audioInputDeviceDef from "./nodes/AudioInputDevice";
import audioOutputDeviceDef from "./nodes/AudioOutputDevice";
import mixerDef from "./nodes/Mixer";
import spectrumAnalyzerDef from "./nodes/SpectrumAnalyzer";
import virtualAudioInputDef from "./nodes/VirtualAudioInput";
import virtualAudioOutputDef from "./nodes/VirtualAudioOutput";
import vstNodeDef from "./nodes/VstNode";
import waveformMonitorDef from "./nodes/WaveformMonitor";

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

/** VST3 plugin info returned by the scan command. */
export interface VstPluginInfo {
  name: string;
  path: string;
  vendor: string;
  numInputs: number;
  numOutputs: number;
  numParams: number;
}

/** A single VST3 parameter descriptor. */
export interface VstParamInfo {
  id: number;
  title: string;
  value: number;
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
  vst: vstNodeDef,
};

export const nodeTypes = Object.fromEntries(
  Object.entries(nodeDefs).map(([k, v]) => [k, v.component]),
) as NodeTypes;

export function serializeNode(node: NodeType): AudioNode {
  const def = nodeDefs[node.type as keyof typeof nodeDefs];
  return (def.toAudioNode as (n: NodeType) => AudioNode)(node);
}

export type NodeType = {
  [K in keyof typeof nodeDefs]: (typeof nodeDefs)[K] extends NodeDefinition<infer TNode>
    ? TNode
    : never;
}[keyof typeof nodeDefs];

export type EdgeType = Edge<AudioEdge>;

export interface AudioGraph {
  nodes: AudioNode[];
  edges: AudioEdge[];
}

export type AudioNode = {
  [K in keyof typeof nodeDefs]: (typeof nodeDefs)[K] extends NodeDefinition<infer TNode>
    ? TNode extends Node<infer TData, infer TType>
      ? { type: TType; data: TData }
      : never
    : never;
}[keyof typeof nodeDefs];

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

export interface AudioGraph {
  nodes: AudioNode[];
  edges: AudioEdge[];
}

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
