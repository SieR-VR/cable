import { Edge, NodeTypes } from "@xyflow/react";
import AudioInputDevice, { AudioInputDeviceNode } from "./nodes/AudioInputDevice";
import AudioOutputDevice, { AudioOutputDeviceNode } from "./nodes/AudioOutputDevice";
import VirtualAudioInput, { VirtualAudioInputNode } from "./nodes/VirtualAudioInput";
import VirtualAudioOutput, { VirtualAudioOutputNode } from "./nodes/VirtualAudioOutput";

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
} satisfies NodeTypes;

export type NodeType =
  | AudioInputDeviceNode
  | AudioOutputDeviceNode
  | VirtualAudioInputNode
  | VirtualAudioOutputNode;

export type EdgeType = Edge<AudioEdge>;

export interface AudioGraph {
  nodes: AudioNode[];
  edges: AudioEdge[];
}

export type AudioNode = {
  type: "audioInputDevice" | "audioOutputDevice" | "virtualAudioInput" | "virtualAudioOutput";
  data: { device: AudioDevice | null; id: string } | { deviceId: string; name: string; id: string };
};

export type AudioEdge = {
  id: string;
  from: string;
  to: string;

  frequency?: number;
  channels?: number;
  bitsPerSample?: number;
};
