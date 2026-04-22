import { Handle, Node, NodeProps, Position } from "@xyflow/react";

export type MixerNodeData = {
  edgeType: string | null;
};

export type MixerNodeType = Node<MixerNodeData, "mixer">;

export default function Mixer({ id }: NodeProps<MixerNodeType>) {
  void id;
  return (
    <div className="bg-gray-700 rounded-lg flex flex-col text-white min-w-48">
      <div className="w-full h-6 bg-orange-500 rounded-t-lg flex items-center text-sm font-bold p-2 drag-handle__custom">
        Mixer
      </div>
      <div className="flex flex-col gap-2 p-2 relative">
        <div className="flex flex-row gap-1 items-center">
          <span className="rounded-md text-xs bg-orange-300 text-orange-900 p-1">sum + clamp</span>
          <span className="rounded-md text-xs bg-gray-500 p-1">passthrough</span>
        </div>
        <Handle
          type="target"
          position={Position.Left}
          id="Mixer-target"
          className="w-4 h-4 bg-orange-400 rounded-full"
        />
        <Handle
          type="source"
          position={Position.Right}
          id="Mixer-source"
          className="w-4 h-4 bg-orange-400 rounded-full"
        />
      </div>
    </div>
  );
}
