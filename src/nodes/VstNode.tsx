import { invoke } from "@tauri-apps/api/core";
import { Node, NodeProps, Position } from "@xyflow/react";
import { useState } from "react";

import { AudioHandle } from "@/components/AudioHandle";
import { NODE_ACCENTS, NodeShell } from "@/components/NodeShell";
import { NodeDefinition } from "@/node-definition";
import { EdgeType, NONE, NodeTypeRecord, audioType, isCompatible } from "@/graph/edge-type";
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
  const [editorError, setEditorError] = useState<string | null>(null);

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
    setEditorError(null);
    try {
      await invoke("node_command", {
        nodeId: id,
        data: { op: "openEditor", pluginPath: data.pluginPath },
      });
    } catch (e) {
      setEditorError(typeof e === "string" ? e : String(e));
    }
  }

  const inputHandles = Array.from({ length: data.numInputs }, (_, i) => `vst-in-${i}`);
  const outputHandles = Array.from({ length: data.numOutputs }, (_, i) => `vst-out-${i}`);
  const maxHandles = Math.max(inputHandles.length, outputHandles.length, 1);

  return (
    <NodeShell accent={NODE_ACCENTS.vst} title="VST Plugin" minWidth="14rem" invalid={(data as any)?.invalid}>
      {/* Plugin selector */}
      <div className="flex gap-1">
        <select
          className="flex-1 text-xs bg-gray-600 text-white rounded px-1 py-0.5 border border-gray-500"
          value={data.pluginPath}
          onChange={(e) => {
            const plugin = vstPluginList.find((p) => p.path === e.target.value);
            if (plugin) {
              updateNode(id, {
                pluginPath: plugin.path,
                numInputs: plugin.numInputs,
                numOutputs: plugin.numOutputs,
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
          <span>
            in:{selectedPlugin.numInputs} out:{selectedPlugin.numOutputs}
          </span>
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

      {/* No-editor feedback */}
      {editorError && <div className="text-xs text-yellow-300 leading-snug">{editorError}</div>}

      {/* I/O handle rows */}
      <div className="relative" style={{ height: `${maxHandles * 24}px` }}>
        {inputHandles.map((handleId, i) => (
          <AudioHandle
            key={handleId}
            type="target"
            position={Position.Left}
            id={handleId}
            style={{ top: `${(i + 0.5) * (100 / maxHandles)}%` }}
          />
        ))}
        {outputHandles.map((handleId, i) => (
          <AudioHandle
            key={handleId}
            type="source"
            position={Position.Right}
            id={handleId}
            style={{ top: `${(i + 0.5) * (100 / maxHandles)}%` }}
          />
        ))}
      </div>
    </NodeShell>
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
  // VST handle keys are dynamic (depend on numInputs / numOutputs), so the
  // static `handles` hint is intentionally omitted — the validator computes
  // them from the node state every run.
  validate: (state, inputs) => {
    const expected: NodeTypeRecord = {};
    const produced: NodeTypeRecord = {};
    let ok = true;
    // Each input handle expects mono audio at whatever rate the upstream
    // claims; we mirror the first concrete input as the produced format.
    let referenceFormat: EdgeType = NONE;
    for (let i = 0; i < state.numInputs; i++) {
      const k = `vst-in-${i}`;
      const t = inputs[k] ?? NONE;
      expected[k] = t;
      if (referenceFormat.kind === "none" && t.kind === "audio") referenceFormat = t;
    }
    for (let i = 0; i < state.numInputs; i++) {
      const k = `vst-in-${i}`;
      if (!isCompatible(inputs[k] ?? NONE, referenceFormat)) ok = false;
    }
    const outFormat: EdgeType =
      referenceFormat.kind === "audio"
        ? audioType(state.channels || referenceFormat.channels, referenceFormat.frequency, referenceFormat.bitsPerSample)
        : NONE;
    for (let i = 0; i < state.numOutputs; i++) {
      produced[`vst-out-${i}`] = outFormat;
    }
    return { expectedInputs: expected, producedOutputs: produced, ok };
  },
};

export default definition;
