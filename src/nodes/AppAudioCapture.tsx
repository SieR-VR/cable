import { useEffect, useState } from "react";
import { Node, NodeProps, Position } from "@xyflow/react";
import { invoke } from "@tauri-apps/api/core";

import { AudioHandle } from "@/components/AudioHandle";
import { NODE_ACCENTS, NodeShell } from "@/components/NodeShell";
import { AppState, useAppStore } from "@/state";
import { WindowInfo } from "@/types";
import { NodeDefinition } from "@/node-definition";
import { NONE, audioType } from "@/graph/edge-type";

export type AppAudioCaptureNodeData = {
  processId: number | null;
  windowTitle: string | null;
  edgeType: string | null;
};

export type AppAudioCaptureNode = Node<AppAudioCaptureNodeData, "appAudioCapture">;

const selector = (id: string) => (store: AppState) => ({
  setWindow: (processId: number, windowTitle: string) => {
    store.updateNode(id, { processId, windowTitle });
  },
});

export function AppAudioCapture({ id, data }: NodeProps<AppAudioCaptureNode>) {
  const { setWindow } = useAppStore(selector(id));
  const [windowList, setWindowList] = useState<WindowInfo[] | null>(null);

  useEffect(() => {
    invoke("get_window_list")
      .then((list) => setWindowList(list as WindowInfo[]))
      .catch(() => setWindowList([]));
  }, []);

  return (
    <NodeShell accent={NODE_ACCENTS.appAudioCapture} title="App Audio Capture">
      {windowList !== null ? (
        <select
          className="w-full p-1 rounded bg-gray-600 text-white text-xs"
          value={data.processId ?? ""}
          onChange={(e) => {
            const selected = windowList.find((w) => w.processId === Number(e.target.value));
            if (selected) setWindow(selected.processId, selected.title);
          }}
        >
          <option value="">-- Select a window --</option>
          {windowList.map((w, i) => (
            <option key={`${w.processId}-${i}`} value={w.processId}>
              {w.title}
            </option>
          ))}
        </select>
      ) : (
        <select className="w-full p-1 rounded bg-gray-600 text-white text-xs" disabled>
          <option>Loading windows...</option>
        </select>
      )}
      {data.processId !== null && data.windowTitle && (
        <div className="text-xs text-gray-300 truncate max-w-48">PID: {data.processId}</div>
      )}
      <AudioHandle type="source" position={Position.Right} id="AppAudioCapture-source" />
    </NodeShell>
  );
}

const definition: NodeDefinition<AppAudioCaptureNode> = {
  component: AppAudioCapture,
  toAudioNode: (node) => ({
    type: "appAudioCapture",
    data: {
      id: node.id,
      processId: node.data.processId ?? 0,
      windowTitle: node.data.windowTitle ?? "",
    },
  }),
  handles: { inputs: [], outputs: ["AppAudioCapture-source"] },
  validate: (state) => {
    // WASAPI loopback capture defaults to the engine's mix format
    // (typically 48k stereo float32). Match the runtime-side default until
    // we plumb negotiated format back to the frontend.
    const t = state.processId ? audioType(2, 48000, 32) : NONE;
    return {
      expectedInputs: {},
      producedOutputs: { "AppAudioCapture-source": t },
      ok: true,
    };
  },
};

export default definition;
