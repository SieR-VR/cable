import { NodeTypes } from "@xyflow/react";
import AudioInputDeviceNode from "./nodes/AudioInputDevice";
import AudioOutputDeviceNode from "./nodes/AudioOutputDevice";

export interface AudioDevice {
  id: string;
  readable_name: string;
  descriptions: string[] | null;
}

export const nodeTypes = {
  audioInputDevice: AudioInputDeviceNode,
  audioOutputDevice: AudioOutputDeviceNode,
} satisfies NodeTypes;
