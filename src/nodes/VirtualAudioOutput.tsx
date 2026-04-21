import { Handle, Node, NodeProps, Position } from "@xyflow/react";

import { AppState, useAppStore } from "@/state";

/**
 * Virtual Audio Output node.
 *
 * Selects a pre-created virtual speaker (render device) from a dropdown.
 * Audio from Windows apps (e.g., a game routing its audio to this virtual
 * speaker) flows out of this node to downstream nodes.
 *
 * Flow UI: has a "source" handle on the right (source node).
 */

export type VirtualAudioOutputNodeData = {
  /** Hex device ID of the selected virtual render device */
  deviceId: string;
  /** Display name (from the selected device) */
  name: string;
  edgeType: string | null;
};

export type VirtualAudioOutputNode = Node<VirtualAudioOutputNodeData, "virtualAudioOutput">;

const selector = (id: string) => (store: AppState) => ({
  setDevice: (deviceId: string, name: string) => store.updateNode(id, { deviceId, name }),
});

export default function VirtualAudioOutput({ id, data }: NodeProps<VirtualAudioOutputNode>) {
  const { setDevice } = useAppStore(selector(id));
  const { driverConnected, virtualDevices } = useAppStore();

  const renderDevices = virtualDevices.filter((d) => d.deviceType === "render");

  return (
    <div className="bg-gray-700 rounded-lg flex flex-col text-white min-w-48">
      {/* Header */}
      <div className="w-full h-6 bg-teal-500 rounded-t-lg flex items-center text-sm font-bold p-2 drag-handle__custom">
        Virtual Speaker (Render)
      </div>
      <div className="flex flex-col gap-2 p-2">
        <select
          className="w-full p-1 rounded bg-gray-500 text-white text-sm"
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
        <div className="flex flex-row gap-1 items-center">
          <span className="rounded-md text-xs bg-teal-300 text-teal-900 p-1">render</span>
          <span className="rounded-md text-xs bg-gray-500 p-1">virtual</span>
        </div>
        <Handle
          type="source"
          position={Position.Right}
          id="VirtualAudioOutput-source"
          className="w-4 h-4 bg-teal-400 rounded-full"
        />
      </div>
    </div>
  );
}
