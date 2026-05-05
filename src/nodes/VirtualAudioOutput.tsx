import { Node, NodeProps, Position } from "@xyflow/react";

import { AudioHandle } from "@/components/AudioHandle";
import { NODE_ACCENTS, NodeShell } from "@/components/NodeShell";
import { AppState, useAppStore } from "@/state";
import { NodeDefinition } from "@/node-definition";
import { NONE, audioType } from "@/graph/edge-type";

export type VirtualAudioOutputNodeData = {
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

export type VirtualAudioOutputNode = Node<VirtualAudioOutputNodeData, "virtualAudioOutput">;

const selector = (id: string) => (store: AppState) => ({
  setDevice: (deviceId: string, name: string, channels?: number, sampleRate?: number, bitsPerSample?: number) =>
    store.updateNode(id, { deviceId, name, channels, sampleRate, bitsPerSample }),
});

export function VirtualAudioOutput({ id, data }: NodeProps<VirtualAudioOutputNode>) {
  const { setDevice } = useAppStore(selector(id));
  const { driverConnected, virtualDevices } = useAppStore();

  const renderDevices = virtualDevices.filter((d) => d.deviceType === "render");

  return (
    <NodeShell accent={NODE_ACCENTS.virtualAudioOutput} title="Virtual Speaker" invalid={(data as any)?.invalid}>
      <select
        className="w-full p-1 rounded bg-gray-600 text-white text-xs"
        value={data.deviceId || ""}
        onChange={(e) => {
          const device = renderDevices.find((d) => d.id === e.target.value);
          setDevice(
            e.target.value,
            device?.name || "",
            device?.channels,
            device?.sampleRate,
            device?.bitsPerSample,
          );
        }}
      >
        <option value="">-- Select virtual speaker --</option>
        {renderDevices.map((device) => (
          <option key={device.id} value={device.id}>
            {device.name}
          </option>
        ))}
      </select>
      {!driverConnected && <div className="text-xs text-yellow-400">Driver not connected</div>}
      {renderDevices.length === 0 && driverConnected && (
        <div className="text-xs text-gray-400">
          No render devices. Create one in the menu panel.
        </div>
      )}
      <AudioHandle type="source" position={Position.Right} id="VirtualAudioOutput-source" />
    </NodeShell>
  );
}

const definition: NodeDefinition<VirtualAudioOutputNode> = {
  component: VirtualAudioOutput,
  toAudioNode: (node) => ({
    type: "virtualAudioOutput",
    data: {
      id: node.id,
      deviceId: node.data.deviceId || "",
      name: node.data.name || "",
    },
  }),
  // VirtualAudioOutput is a SOURCE: Windows applications write audio to the
  // virtual speaker and this node reads from the ring buffer, forwarding audio
  // downstream in the graph. The "source" handle emits audio; there is no
  // input handle.
  handles: { inputs: [], outputs: ["VirtualAudioOutput-source"] },
  validate: (state) => {
    // Use the device's format preset for the produced output type.
    // Falls back to the driver default (48kHz / stereo / 32-bit) when no
    // preset is stored on the node data.
    const channels = state.channels ?? 2;
    const sampleRate = state.sampleRate ?? 48000;
    const bitsPerSample = state.bitsPerSample ?? 32;
    const produced = state.deviceId ? audioType(channels, sampleRate, bitsPerSample) : NONE;
    return {
      expectedInputs: {},
      producedOutputs: { "VirtualAudioOutput-source": produced },
      ok: true,
    };
  },
};

export default definition;
