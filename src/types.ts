import { Edge, Node, NodeTypes } from "@xyflow/react";

import { EdgeType as GraphEdgeType } from "./graph/edge-type";
import { NodeDefinition } from "./node-definition";
import appAudioCaptureDef from "./nodes/AppAudioCapture";
import audioInputDeviceDef from "./nodes/AudioInputDevice";
import audioOutputDeviceDef from "./nodes/AudioOutputDevice";
import channelSplitDef from "./nodes/ChannelSplit";
import compressorDef from "./nodes/Compressor";
import delayDef from "./nodes/Delay";
import echoDef from "./nodes/Echo";
import gainDef from "./nodes/Gain";
import mixerDef from "./nodes/Mixer";
import reverbDef from "./nodes/Reverb";
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
  gain: gainDef,
  channelSplit: channelSplitDef,
  delay: delayDef,
  compressor: compressorDef,
  reverb: reverbDef,
  echo: echoDef,
  vst: vstNodeDef,
};

let _nodeTypes: NodeTypes | null = null;
export const nodeTypes: NodeTypes = new Proxy({} as NodeTypes, {
  get(_t, prop: string) {
    if (!_nodeTypes) {
      _nodeTypes = Object.fromEntries(
        Object.entries(nodeDefs).map(([k, v]) => [k, v.component]),
      ) as NodeTypes;
    }
    return _nodeTypes[prop];
  },
  ownKeys() {
    if (!_nodeTypes) {
      _nodeTypes = Object.fromEntries(
        Object.entries(nodeDefs).map(([k, v]) => [k, v.component]),
      ) as NodeTypes;
    }
    return Reflect.ownKeys(_nodeTypes);
  },
  getOwnPropertyDescriptor(_t, prop) {
    if (!_nodeTypes) {
      _nodeTypes = Object.fromEntries(
        Object.entries(nodeDefs).map(([k, v]) => [k, v.component]),
      ) as NodeTypes;
    }
    return Object.getOwnPropertyDescriptor(_nodeTypes, prop);
  },
});

export function serializeNode(node: NodeType): AudioNode {
  const def = nodeDefs[node.type as keyof typeof nodeDefs];
  return (def.toAudioNode as (n: NodeType) => AudioNode)(node);
}

export function serializeEdge(edge: EdgeType): AudioEdge {
  const t = edge.data?.edgeType;
  const audio = t && t.kind === "audio" ? t : null;
  return {
    id: edge.id,
    from: edge.source,
    fromHandle: edge.sourceHandle ?? undefined,
    to: edge.target,
    toHandle: edge.targetHandle ?? undefined,
    edgeType: t,
    invalid: edge.data?.invalid,
    frequency: audio?.frequency ?? edge.data?.frequency,
    channels: audio?.channels ?? edge.data?.channels,
    bitsPerSample: audio?.bitsPerSample ?? edge.data?.bitsPerSample,
  };
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
  /** Source handle ID (e.g. "ch-0", "ch-1" for ChannelSplit node) */
  fromHandle?: string;
  to: string;
  /** Target handle ID (e.g. "input-a", "input-b" for Mixer node) */
  toHandle?: string;

  /**
   * Phase 1: structured edge type carried alongside the legacy flat fields.
   * Determined by the source node's `producedOutputs[fromHandle]` once the
   * Phase 4 validation engine is wired up. UI code may read this in addition
   * to the flat fields below.
   */
  edgeType?: GraphEdgeType;
  /** Set by the validation engine when the sink's expected input doesn't match `edgeType`. */
  invalid?: boolean;

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
