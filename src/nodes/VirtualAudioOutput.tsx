import { Node, NodeProps, Position } from "@xyflow/react";

import { AudioHandle } from "@/components/AudioHandle";
import { NODE_ACCENTS, NodeShell } from "@/components/NodeShell";
import { AppState, useAppStore } from "@/state";
import { NodeDefinition } from "@/node-definition";

export type VirtualAudioOutputNodeData = {
  deviceId: string;
  name: string;
  edgeType: string | null;
};

export type VirtualAudioOutputNode = Node<VirtualAudioOutputNodeData, "virtualAudioOutput">;

const selector = (id: string) => (store: AppState) => ({
  setDevice: (deviceId: string, name: string) => store.updateNode(id, { deviceId, name }),
});

export function VirtualAudioOutput({ id, data }: NodeProps<VirtualAudioOutputNode>) {
  const { setDevice } = useAppStore(selector(id));
  const { driverConnected, virtualDevices } = useAppStore();

  const renderDevices = virtualDevices.filter((d) => d.deviceType === "render");

  return (
    <NodeShell accent={NODE_ACCENTS.virtualAudioOutput} title="Virtual Speaker">
      <select
        className="w-full p-1 rounded bg-gray-600 text-white text-xs"
        value={data.deviceId || ""}
        onChange={(e) => {
          const device = renderDevices.find((d) => d.id === e.target.value);
          setDevice(e.target.value, device?.name || "");
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
};

export default definition;
