import { Node, NodeProps, Position } from "@xyflow/react";

import { AudioHandle } from "@/components/AudioHandle";
import { BluetoothBadge, BluetoothBatteryWidget, useBluetoothInfo } from "@/components/BluetoothBadge";
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
  const { availableAudioInputDevices, virtualDevices } = useAppStore();
  const { setDevice } = useAppStore(selector(id));
  const btInfo = useBluetoothInfo(data?.device ?? null);

  // Exclude Cable virtual audio devices from the dropdown — those are managed
  // through the virtual device panel and should not be selectable as plain inputs.
  // cpal device IDs use the "wasapi:{endpoint_id}" format, while endpointId from the
  // driver uses the raw "{endpoint_id}" format, so we prepend the host prefix to match.
  const virtualEndpointIds = new Set(
    virtualDevices
      .map((d) => (d.endpointId ? `wasapi:${d.endpointId}` : null))
      .filter((id): id is string => Boolean(id)),
  );
  const filteredDevices = availableAudioInputDevices?.filter(
    (d) => !virtualEndpointIds.has(d.id),
  );

  return (
    <NodeShell accent={NODE_ACCENTS.audioInputDevice} title="Audio Input" invalid={(data as any)?.invalid}>
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
            <option value="">-- Select an audio input device --</option>
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
      {!filteredDevices && (
        <div className="text-xs text-gray-400">{"Loading..."}</div>
      )}
      <BluetoothBadge info={btInfo} />
      <BluetoothBatteryWidget info={btInfo} />
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

