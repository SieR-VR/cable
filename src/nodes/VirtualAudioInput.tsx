import { Handle, Node, NodeProps, Position } from "@xyflow/react";

import { AppState, useAppStore } from "@/state";

/**
 * Virtual Audio Input node.
 *
 * Selects a pre-created virtual microphone (capture device) from a dropdown.
 * Audio from upstream nodes flows into this node and becomes available to
 * Windows apps (Discord, OBS, etc.).
 *
 * Flow UI: has a "target" handle on the left (sink node).
 */

export type VirtualAudioInputNodeData = {
  /** Hex device ID of the selected virtual capture device */
  deviceId: string;
  /** Display name (from the selected device) */
  name: string;
  edgeType: string | null;
};

export type VirtualAudioInputNode = Node<VirtualAudioInputNodeData, "virtualAudioInput">;

const selector = (id: string) => (store: AppState) => ({
  setDevice: (deviceId: string, name: string) => store.updateNode(id, { deviceId, name }),
});

export default function VirtualAudioInput({ id, data }: NodeProps<VirtualAudioInputNode>) {
  const { setDevice } = useAppStore(selector(id));
  const { driverConnected, virtualDevices } = useAppStore();

  const captureDevices = virtualDevices.filter((d) => d.deviceType === "capture");

  return (
    <div className="bg-gray-700 rounded-lg flex flex-col text-white min-w-48">
      {/* Header */}
      <div className="w-full h-6 bg-purple-500 rounded-t-lg flex items-center text-sm font-bold p-2 drag-handle__custom">
        Virtual Mic (Capture)
      </div>
      <div className="flex flex-col gap-2 p-2">
        <select
          className="w-full p-1 rounded bg-gray-500 text-white text-sm"
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
        <div className="flex flex-row gap-1 items-center">
          <span className="rounded-md text-xs bg-purple-300 text-purple-900 p-1">capture</span>
          <span className="rounded-md text-xs bg-gray-500 p-1">virtual</span>
        </div>
        <Handle
          type="target"
          position={Position.Left}
          id="VirtualAudioInput-target"
          className="w-4 h-4 bg-purple-400 rounded-full"
        />
      </div>
    </div>
  );
}
