import { Node, NodeProps, Position } from "@xyflow/react";

import { AudioHandle } from "@/components/AudioHandle";
import { NODE_ACCENTS, NodeShell } from "@/components/NodeShell";
import { formatAudioEdgeType } from "@/lib/utils";
import { AppState, useAppStore } from "@/state";
import { AudioDevice } from "@/types";
import { NodeDefinition } from "@/node-definition";
import { NONE, audioType, isCompatible } from "@/graph/edge-type";

export type AudioOutputDeviceNodeData = {
  device: AudioDevice | null;
  edgeType: string | null;
};

export type AudioOutputDeviceNode = Node<AudioOutputDeviceNodeData, "audioOutputDevice">;

const selector = (id: string) => (store: AppState) => ({
  setDevice: (device: AudioDevice | null) => {
    const edgeType =
      device && formatAudioEdgeType(device.frequency, device.channels, device.bitsPerSample);

    store.updateNode(id, { device, edgeType });
  },
});

export function AudioOutputDevice({ id, data }: NodeProps<AudioOutputDeviceNode>) {
  void data;
  const { availableAudioOutputDevices } = useAppStore();
  const { setDevice } = useAppStore(selector(id));

  return (
    <NodeShell accent={NODE_ACCENTS.audioOutputDevice} title="Audio Output">
      <select
        className="w-full p-1 rounded bg-gray-600 text-white text-xs"
        onChange={(e) => {
          setDevice(
            availableAudioOutputDevices?.find((device) => device.id === e.target.value) || null,
          );
        }}
      >
        {availableAudioOutputDevices ? (
          <>
            <option value="">-- Select an audio output device --</option>
            {availableAudioOutputDevices.map((device) => (
              <option key={device.id} value={device.id}>
                {device.descriptions?.join("\n")}
              </option>
            ))}
          </>
        ) : (
          <option disabled>Loading devices...</option>
        )}
      </select>
      {!availableAudioOutputDevices && (
        <div className="text-xs text-gray-400">{"Loading..."}</div>
      )}
      <AudioHandle type="target" position={Position.Left} id="AudioOutputDevice-target" />
    </NodeShell>
  );
}

const definition: NodeDefinition<AudioOutputDeviceNode> = {
  component: AudioOutputDevice,
  toAudioNode: (node) => ({
    type: "audioOutputDevice",
    data: { id: node.id, device: node.data.device },
  }),
  handles: { inputs: ["AudioOutputDevice-target"], outputs: [] },
  validate: (state, inputs) => {
    const expected = state.device
      ? audioType(state.device.channels, state.device.frequency, state.device.bitsPerSample)
      : NONE;
    const actual = inputs["AudioOutputDevice-target"] ?? NONE;
    return {
      expectedInputs: { "AudioOutputDevice-target": expected },
      producedOutputs: {},
      ok: isCompatible(actual, expected),
    };
  },
};

export default definition;
