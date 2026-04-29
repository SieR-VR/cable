import { Node, NodeProps, Position } from "@xyflow/react";

import { AudioHandle } from "@/components/AudioHandle";
import { NODE_ACCENTS, NodeShell } from "@/components/NodeShell";
import { AppState, useAppStore } from "@/state";
import { NodeDefinition } from "@/node-definition";
import { NONE, audioType } from "@/graph/edge-type";

export type VirtualAudioInputNodeData = {
  deviceId: string;
  name: string;
  edgeType: string | null;
};

export type VirtualAudioInputNode = Node<VirtualAudioInputNodeData, "virtualAudioInput">;

const selector = (id: string) => (store: AppState) => ({
  setDevice: (deviceId: string, name: string) => store.updateNode(id, { deviceId, name }),
});

export function VirtualAudioInput({ id, data }: NodeProps<VirtualAudioInputNode>) {
  const { setDevice } = useAppStore(selector(id));
  const { driverConnected, virtualDevices } = useAppStore();

  const captureDevices = virtualDevices.filter((d) => d.deviceType === "capture");

  return (
    <NodeShell accent={NODE_ACCENTS.virtualAudioInput} title="Virtual Mic">
      <select
        className="w-full p-1 rounded bg-gray-600 text-white text-xs"
        value={data.deviceId || ""}
        onChange={(e) => {
          const device = captureDevices.find((d) => d.id === e.target.value);
          setDevice(e.target.value, device?.name || "");
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
  handles: { inputs: [], outputs: ["VirtualAudioInput-source"] },
  validate: (state) => {
    // Driver currently negotiates a fixed engine format; until that is exposed
    // back to the UI we report `none` when no device is bound and a default
    // 48k stereo 32-bit float when one is.
    const t = state.deviceId ? audioType(2, 48000, 32) : NONE;
    return {
      expectedInputs: {},
      producedOutputs: { "VirtualAudioInput-source": t },
      ok: true,
    };
  },
};

export default definition;
