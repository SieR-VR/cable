import { Handle, Node, NodeProps, Position } from "@xyflow/react";
import { AppState, useAppStore } from "@/state";

/**
 * Virtual Audio Output node.
 *
 * Creates a virtual speaker (render device) in Windows. Audio from Windows apps
 * (e.g., a game routing its audio to this virtual speaker) flows out of this node
 * to downstream nodes (e.g., a real audio output device).
 *
 * Flow UI: has a "source" handle on the right (source node).
 */

export type VirtualAudioOutputNodeData = {
  name: string;
  edgeType: string | null;
};

export type VirtualAudioOutputNode = Node<
  VirtualAudioOutputNodeData,
  "virtualAudioOutput"
>;

const selector = (id: string) => (store: AppState) => ({
  setName: (name: string) => store.updateNode(id, { name }),
});

export default function VirtualAudioOutput({
  id,
  data,
}: NodeProps<VirtualAudioOutputNode>) {
  const { setName } = useAppStore(selector(id));
  const { driverConnected } = useAppStore();

  return (
    <div className="bg-gray-700 rounded-lg flex flex-col text-white min-w-48">
      {/* Header */}
      <div className="w-full h-6 bg-teal-500 rounded-t-lg flex items-center text-sm font-bold p-2 drag-handle__custom">
        Virtual Speaker (Render)
      </div>
      <div className="flex flex-col gap-2 p-2">
        <input
          type="text"
          className="w-full p-1 rounded bg-gray-500 text-white text-sm"
          placeholder="Device name..."
          value={data.name || ""}
          onChange={(e) => setName(e.target.value)}
        />
        {!driverConnected && (
          <div className="text-xs text-yellow-400">Driver not connected</div>
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
