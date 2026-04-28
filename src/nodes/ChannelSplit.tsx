import { Node, NodeProps, Position } from "@xyflow/react";

import { AudioHandle } from "@/components/AudioHandle";
import { NODE_ACCENTS, NodeShell } from "@/components/NodeShell";
import { NodeDefinition } from "@/node-definition";

export type ChannelSplitNodeData = {
  edgeType: string | null;
};

export type ChannelSplitNodeType = Node<ChannelSplitNodeData, "channelSplit">;

const OUTPUTS = [
  { id: "ch-0", label: "L / Ch 0" },
  { id: "ch-1", label: "R / Ch 1" },
] as const;

const ROW_HEIGHT = 24;

export function ChannelSplit({ id }: NodeProps<ChannelSplitNodeType>) {
  void id;
  return (
    <NodeShell accent={NODE_ACCENTS.channelSplit} title="Channel Split">
      {/*
        Negative horizontal margin mirrors the Mixer pattern: removes NodeShell's
        `p-2` body padding so handles sit flush with the card edges.
      */}
      <div
        className="relative -mx-2"
        style={{ height: OUTPUTS.length * ROW_HEIGHT }}
      >
        {OUTPUTS.map((out, i) => {
          const top = i * ROW_HEIGHT + ROW_HEIGHT / 2;
          return (
            <div key={out.id}>
              <span
                className="absolute text-xs text-gray-300"
                style={{ top, right: 16, transform: "translateY(-50%)" }}
              >
                {out.label}
              </span>
              <AudioHandle
                type="source"
                position={Position.Right}
                id={out.id}
                style={{ top }}
              />
            </div>
          );
        })}
      </div>
      <AudioHandle type="target" position={Position.Left} id="ChannelSplit-target" />
    </NodeShell>
  );
}

const definition: NodeDefinition<ChannelSplitNodeType> = {
  component: ChannelSplit,
  toAudioNode: (node) => ({
    type: "channelSplit",
    data: { id: node.id },
  }),
};

export default definition;
