import { Node, NodeProps, Position } from "@xyflow/react";

import { AudioHandle } from "@/components/AudioHandle";
import { NodeDefinition } from "@/node-definition";

export type MixerNodeData = {
  edgeType: string | null;
};

export type MixerNodeType = Node<MixerNodeData, "mixer">;

export function Mixer({ id }: NodeProps<MixerNodeType>) {
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
        {/* Input A */}
        <div className="flex items-center gap-2 h-6 relative">
          <AudioHandle
            type="target"
            position={Position.Left}
            id="input-a"
            className="!static !transform-none"
          />
          <span className="text-xs text-gray-300">A</span>
        </div>
        {/* Input B */}
        <div className="flex items-center gap-2 h-6 relative">
          <AudioHandle
            type="target"
            position={Position.Left}
            id="input-b"
            className="!static !transform-none"
          />
          <span className="text-xs text-gray-300">B</span>
        </div>
        <AudioHandle
          type="source"
          position={Position.Right}
          id="Mixer-source"
        />
      </div>
    </div>
  );
}

const definition: NodeDefinition<MixerNodeType> = {
  component: Mixer,
  toAudioNode: (node) => ({
    type: "mixer",
    data: { id: node.id },
  }),
};

export default definition;
