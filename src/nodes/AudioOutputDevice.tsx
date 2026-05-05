import { Node, NodeProps, Position } from "@xyflow/react";

import { AudioHandle } from "@/components/AudioHandle";
import { BluetoothBatteryWidget, useBluetoothInfo } from "@/components/BluetoothBadge";
import { NODE_ACCENTS, NodeShell } from "@/components/NodeShell";
import { NONE, audioType, isCompatible } from "@/graph/edge-type";
import { formatAudioEdgeType } from "@/lib/utils";
import { NodeDefinition } from "@/node-definition";
import { AppState, useAppStore } from "@/state";
import { AudioDevice } from "@/types";

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
  const { availableAudioOutputDevices, virtualDevices } = useAppStore();
  const { setDevice } = useAppStore(selector(id));
  const btInfo = useBluetoothInfo(data?.device ?? null);

  // Exclude Cable virtual audio devices from the dropdown — those are managed
  // through the virtual device panel and should not be selectable as plain outputs.
  // cpal device IDs use the "wasapi:{endpoint_id}" format, while endpointId from the
  // driver uses the raw "{endpoint_id}" format, so we prepend the host prefix to match.
  const virtualEndpointIds = new Set(
    virtualDevices
      .map((d) => (d.endpointId ? `wasapi:${d.endpointId}` : null))
      .filter((id): id is string => Boolean(id)),
  );
  const filteredDevices = availableAudioOutputDevices?.filter(
    (d) => !virtualEndpointIds.has(d.id),
  );

  return (
    <NodeShell
      accent={NODE_ACCENTS.audioOutputDevice}
      title="Audio Output"
      invalid={(data as any)?.invalid}
    >
      <select
        className="w-full p-1 rounded bg-gray-600 text-white text-xs"
        value={data?.device?.id ?? ""}
        onChange={(e) => {
          setDevice(
            filteredDevices?.find((device) => device.id === e.target.value) || null,
          );
        }}
      >
        {filteredDevices ? (
          <>
            <option value="">-- Select an audio output device --</option>
            {filteredDevices.map((device) => (
              <option key={device.id} value={device.id}>
                {device.descriptions?.join("\n")}
              </option>
            ))}
          </>
        ) : (
          <option disabled>Loading devices...</option>
        )}
      </select>
      {!filteredDevices && <div className="text-xs text-gray-400">{"Loading..."}</div>}
      <BluetoothBatteryWidget info={btInfo} />
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
