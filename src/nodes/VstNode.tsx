import { invoke } from "@tauri-apps/api/core";
import { Handle, Node, NodeProps, Position } from "@xyflow/react";
import { useState } from "react";

import { NodeDefinition } from "@/node-definition";
import { useAppStore } from "@/state";

export type VstNodeData = {
  pluginPath: string;
  numInputs: number;
  numOutputs: number;
  channels: number;
  params: number[];
};

export type VstNodeType = Node<VstNodeData, "vst">;

export function VstNode({ id, data }: NodeProps<VstNodeType>) {
  const vstPluginList = useAppStore((s) => s.vstPluginList);
  const scanVstPlugins = useAppStore((s) => s.scanVstPlugins);
  const updateNode = useAppStore((s) => s.updateNode);

  const [scanning, setScanning] = useState(false);

  const selectedPlugin = vstPluginList.find((p) => p.path === data.pluginPath) ?? null;

  async function handleScan() {
    setScanning(true);
    try {
      await scanVstPlugins();
    } finally {
      setScanning(false);
    }
  }

  async function handleOpenEditor() {
    await invoke("open_vst_editor", { nodeId: id, pluginPath: data.pluginPath });
  }

  const inputHandles = Array.from({ length: data.numInputs }, (_, i) => `vst-in-${i}`);
  const outputHandles = Array.from({ length: data.numOutputs }, (_, i) => `vst-out-${i}`);
  const maxHandles = Math.max(inputHandles.length, outputHandles.length, 1);

  return (
    <div className="bg-gray-700 rounded-lg flex flex-col text-white min-w-56">
      <div className="w-full h-6 bg-violet-500 rounded-t-lg flex items-center text-sm font-bold p-2 drag-handle__custom">
        VST Plugin
      </div>
      <div className="flex flex-col gap-2 p-2">
        {/* Plugin selector */}
        <div className="flex gap-1">
          <select
            className="flex-1 text-xs bg-gray-600 text-white rounded px-1 py-0.5 border border-gray-500"
            value={data.pluginPath}
            onChange={async (e) => {
              const plugin = vstPluginList.find((p) => p.path === e.target.value);
              if (plugin) {
                updateNode(id, {
                  pluginPath: plugin.path,
                  numInputs: plugin.numInputs,
                  numOutputs: plugin.numOutputs,
                });
                // Pre-extract ctrl_cid so the editor can be opened without Apply
                await invoke("create_node", {
                  node: {
                    type: "vst",
                    data: {
                      id,
                      pluginPath: plugin.path,
                      numInputs: plugin.numInputs,
                      numOutputs: plugin.numOutputs,
                      channels: data.channels || 2,
                      params: data.params || [],
                    },
                  },
                });
              } else {
                updateNode(id, { pluginPath: "" });
              }
            }}
          >
            <option value="">-- Select Plugin --</option>
            {vstPluginList.map((p) => (
              <option key={p.path} value={p.path}>
                {p.name}
              </option>
            ))}
          </select>
          <button
            className="text-xs bg-gray-600 hover:bg-gray-500 rounded px-2 py-0.5 border border-gray-500"
            onClick={handleScan}
            disabled={scanning}
            title="Scan for VST3 plugins"
          >
            {scanning ? "…" : "Scan"}
          </button>
        </div>

        {/* Plugin info */}
        {selectedPlugin && (
          <div className="text-xs text-gray-400 flex gap-2">
            <span>{selectedPlugin.vendor || "Unknown vendor"}</span>
            <span>·</span>
            <span>in:{selectedPlugin.numInputs} out:{selectedPlugin.numOutputs}</span>
          </div>
        )}

        {/* Open editor button */}
        {selectedPlugin && (
          <button
            className="text-xs bg-violet-600 hover:bg-violet-500 rounded px-2 py-1"
            onClick={handleOpenEditor}
          >
            Open Editor
          </button>
        )}

        {/* I/O handle rows */}
        <div className="relative" style={{ height: `${maxHandles * 24}px` }}>
          {inputHandles.map((handleId, i) => (
            <Handle
              key={handleId}
              type="target"
              position={Position.Left}
              id={handleId}
              style={{ top: `${(i + 0.5) * (100 / maxHandles)}%` }}
              className="w-3 h-3 bg-violet-400 rounded-full"
            />
          ))}
          {outputHandles.map((handleId, i) => (
            <Handle
              key={handleId}
              type="source"
              position={Position.Right}
              id={handleId}
              style={{ top: `${(i + 0.5) * (100 / maxHandles)}%` }}
              className="w-3 h-3 bg-violet-400 rounded-full"
            />
          ))}
        </div>
      </div>
    </div>
  );
}

const definition: NodeDefinition<VstNodeType> = {
  component: VstNode,
  toAudioNode: (node) => ({
    type: "vst",
    data: {
      id: node.id,
      pluginPath: node.data.pluginPath,
      numInputs: node.data.numInputs,
      numOutputs: node.data.numOutputs,
      channels: node.data.channels,
      params: node.data.params,
    },
  }),
};

export default definition;
