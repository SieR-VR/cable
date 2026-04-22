import { useEffect, useState } from "react";
import { Handle, Node, NodeProps, Position } from "@xyflow/react";
import { invoke } from "@tauri-apps/api/core";

import { AppState, useAppStore } from "@/state";
import { WindowInfo } from "@/types";

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

export default function AppAudioCapture({ id, data }: NodeProps<AppAudioCaptureNode>) {
  const { setWindow } = useAppStore(selector(id));
  const [windowList, setWindowList] = useState<WindowInfo[] | null>(null);

  useEffect(() => {
    invoke("get_window_list")
      .then((list) => setWindowList(list as WindowInfo[]))
      .catch(() => setWindowList([]));
  }, []);

  return (
    <div className="bg-gray-700 rounded-lg flex flex-col text-white">
      <div className="w-full h-6 bg-orange-500 rounded-t-lg flex items-center text-sm font-bold p-2 drag-handle__custom">
        App Audio Capture
      </div>
      <div className="flex flex-col gap-2 p-2">
        <div className="w-full flex flex-col">
          {windowList !== null ? (
            <select
              className="w-full p-1 rounded bg-gray-500"
              value={data.processId ?? ""}
              onChange={(e) => {
                const selected = windowList.find(
                  (w) => w.processId === Number(e.target.value),
                );
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
            <select className="w-full p-1 rounded bg-gray-500" disabled>
              <option>Loading windows...</option>
            </select>
          )}
        </div>
        {data.processId !== null && data.windowTitle && (
          <div className="text-xs text-gray-300 truncate max-w-48">
            PID: {data.processId}
          </div>
        )}
        <Handle
          type="source"
          position={Position.Right}
          id="AppAudioCapture-source"
          className="w-4 h-4 bg-green-500 rounded-full"
        />
      </div>
    </div>
  );
}
