import { Node, NodeProps, Position } from "@xyflow/react";

import { AudioHandle } from "@/components/AudioHandle";
import { NODE_ACCENTS, NodeShell } from "@/components/NodeShell";
import { formatAudioEdgeType } from "@/lib/utils";
import { AppState, useAppStore } from "@/state";
import { AudioDevice } from "@/types";
import { NodeDefinition } from "@/node-definition";
import { NONE, audioType } from "@/graph/edge-type";

export type AudioInputDeviceNodeData = {
  device: AudioDevice | null;
  edgeType: string | null;
};

export type AudioInputDeviceNode = Node<AudioInputDeviceNodeData, "audioInputDevice">;

const selector = (id: string) => (store: AppState) => ({
  setDevice: (device: AudioDevice | null) => {
    const edgeType =
      device && formatAudioEdgeType(device.frequency, device.channels, device.bitsPerSample);

    store.updateNode(id, { device, edgeType });
  },
});

export function AudioInputDevice({ id, data }: NodeProps<AudioInputDeviceNode>) {
  void data;
  const { availableAudioInputDevices } = useAppStore();
  const { setDevice } = useAppStore(selector(id));

  return (
    <NodeShell accent={NODE_ACCENTS.audioInputDevice} title="Audio Input" invalid={(data as any)?.invalid}>
      <select
        className="w-full p-1 rounded bg-gray-600 text-white text-xs"
        onChange={(e) => {
          setDevice(
            availableAudioInputDevices?.find((device) => device.id === e.target.value) || null,
          );
        }}
      >
        {availableAudioInputDevices ? (
          <>
            <option value="">-- Select an audio input device --</option>
            {availableAudioInputDevices.map((device) => (
              <option key={device.id} value={device.id}>
                {device.descriptions?.join("\n")}
              </option>
            ))}
          </>
        ) : (
          <option disabled>Loading devices...</option>
        )}
      </select>
      {!availableAudioInputDevices && (
        <div className="text-xs text-gray-400">{"Loading..."}</div>
      )}
      <AudioHandle type="source" position={Position.Right} id="AudioInputDevice-source" />
    </NodeShell>
  );
}

const definition: NodeDefinition<AudioInputDeviceNode> = {
  component: AudioInputDevice,
  toAudioNode: (node) => ({
    type: "audioInputDevice",
    data: { id: node.id, device: node.data.device },
  }),
  handles: { inputs: [], outputs: ["AudioInputDevice-source"] },
  validate: (state) => {
    const t = state.device
      ? audioType(state.device.channels, state.device.frequency, state.device.bitsPerSample)
      : NONE;
    return {
      expectedInputs: {},
      producedOutputs: { "AudioInputDevice-source": t },
      ok: true,
    };
  },
};

export default definition;

