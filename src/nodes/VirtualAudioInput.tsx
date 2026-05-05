import { Node, NodeProps, Position } from "@xyflow/react";

import { AudioHandle } from "@/components/AudioHandle";
import { NODE_ACCENTS, NodeShell } from "@/components/NodeShell";
import { AppState, useAppStore } from "@/state";
import { NodeDefinition } from "@/node-definition";
import { NONE, audioType, isCompatible } from "@/graph/edge-type";

export type VirtualAudioInputNodeData = {
  deviceId: string;
  name: string;
  edgeType: string | null;
  /** Channel count from the device's format preset. */
  channels?: number;
  /** Sample rate from the device's format preset. */
  sampleRate?: number;
  /** Bits per sample from the device's format preset. */
  bitsPerSample?: number;
};

export type VirtualAudioInputNode = Node<VirtualAudioInputNodeData, "virtualAudioInput">;

const selector = (id: string) => (store: AppState) => ({
  setDevice: (deviceId: string, name: string, channels?: number, sampleRate?: number, bitsPerSample?: number) =>
    store.updateNode(id, { deviceId, name, channels, sampleRate, bitsPerSample }),
});

export function VirtualAudioInput({ id, data }: NodeProps<VirtualAudioInputNode>) {
  const { setDevice } = useAppStore(selector(id));
  const { driverConnected, virtualDevices } = useAppStore();

  const captureDevices = virtualDevices.filter((d) => d.deviceType === "capture");

  return (
    <NodeShell accent={NODE_ACCENTS.virtualAudioInput} title="Virtual Mic" invalid={(data as any)?.invalid}>
      <select
        className="w-full p-1 rounded bg-gray-600 text-white text-xs"
        value={data.deviceId || ""}
        onChange={(e) => {
          const device = captureDevices.find((d) => d.id === e.target.value);
          setDevice(
            e.target.value,
            device?.name || "",
            device?.channels,
            device?.sampleRate,
            device?.bitsPerSample,
          );
        }}
      >
        <option value="">-- Select virtual mic --</option>
        {captureDevices.map((device) => (
          <option key={device.id} value={device.id}>
            {device.name}
          </option>
        ))}
      </select>
      {!driverConnected && <div className="text-xs text-yellow-400">Driver not connected</div>}
      {captureDevices.length === 0 && driverConnected && (
        <div className="text-xs text-gray-400">
          No capture devices. Create one in the menu panel.
        </div>
      )}
      <AudioHandle type="target" position={Position.Left} id="VirtualAudioInput-target" />
    </NodeShell>
  );
}

const definition: NodeDefinition<VirtualAudioInputNode> = {
  component: VirtualAudioInput,
  toAudioNode: (node) => ({
    type: "virtualAudioInput",
    data: {
      id: node.id,
      deviceId: node.data.deviceId || "",
      name: node.data.name || "",
    },
  }),
  // VirtualAudioInput is a SINK: receives audio from the graph and writes
  // it to the virtual microphone ring buffer. The "target" handle accepts
  // incoming audio; there is no output handle.
  handles: { inputs: ["VirtualAudioInput-target"], outputs: [] },
  validate: (state, inputs) => {
    // Use the device's format preset for the expected input type.
    // Falls back to the driver default (48kHz / stereo / 32-bit) when no
    // preset is stored on the node data.
    const channels = state.channels ?? 2;
    const sampleRate = state.sampleRate ?? 48000;
    const bitsPerSample = state.bitsPerSample ?? 32;
    const expected = state.deviceId ? audioType(channels, sampleRate, bitsPerSample) : NONE;
    const actual = inputs["VirtualAudioInput-target"] ?? NONE;
    return {
      expectedInputs: { "VirtualAudioInput-target": expected },
      producedOutputs: {},
      ok: !state.deviceId || isCompatible(actual, expected),
    };
  },
};

export default definition;
