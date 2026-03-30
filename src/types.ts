import { Edge, NodeTypes } from "@xyflow/react";
import AudioInputDevice, {
  AudioInputDeviceNode,
} from "./nodes/AudioInputDevice";
import AudioOutputDevice, {
  AudioOutputDeviceNode,
} from "./nodes/AudioOutputDevice";

export interface AudioDevice {
  id: string;
  readableName: string;
  descriptions: string[] | null;

  frequency: number;
  channels: number;
  bitsPerSample: number;
}

export const nodeTypes = {
  audioInputDevice: AudioInputDevice,
  audioOutputDevice: AudioOutputDevice,
} satisfies NodeTypes;

export type NodeType = AudioInputDeviceNode | AudioOutputDeviceNode;

export type EdgeType = Edge<AudioEdge>;

export interface AudioGraph {
  nodes: AudioNode[];
  edges: AudioEdge[];
}

export type AudioNode = {
  type: "audioInputDevice" | "audioOutputDevice";
  data: { device: AudioDevice | null; id: string };
};

export type AudioEdge = {
  from: string;
  to: string;

  frequency?: number;
  channels?: number;
  bitsPerSample?: number;
};
