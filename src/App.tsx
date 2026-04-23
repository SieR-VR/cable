import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { ReactFlow, Background, BackgroundVariant, Panel, ReactFlowInstance } from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import { Cpu, Play, Save, Square, Zap } from "lucide-react";
import { MouseEvent as ReactMouseEvent, useCallback, useEffect, useState } from "react";

import { ContextMenu } from "./components/ContextMenu";
import Menu from "./components/Menu";

import { useAppStore } from "./state";
import { AudioGraph, CableGraphFile, EdgeType, NodeType, nodeTypes, serializeNode } from "./types";

function App() {
  const {
    contextMenuOpen,
    setContextMenuOpen,
    initializeApp,
    selectedAudioHost,
    driverConnected,
    nodes,
    edges,
    onNodesChange,
    onEdgesChange,
    onConnect,
    loadGraph,
    startRenderPolling,
    stopRenderPolling,
  } = useAppStore();

  const [isRuntimeEnabled, setIsRuntimeEnabled] = useState(false);
  const [applyStatus, setApplyStatus] = useState<string | null>(null);
  const [reactFlowInstance, setReactFlowInstance] = useState<ReactFlowInstance<
    NodeType,
    EdgeType
  > | null>(null);

  const onPaneContextMenu = useCallback(
    (event: MouseEvent | ReactMouseEvent<Element, MouseEvent>) => {
      event.preventDefault();

      const screenPosition = { x: event.clientX, y: event.clientY };
      const flowPosition =
        reactFlowInstance?.screenToFlowPosition(screenPosition) || screenPosition;

      setContextMenuOpen(true, screenPosition, flowPosition);
    },
    [reactFlowInstance, setContextMenuOpen],
  );

  const onNodeContextMenu = useCallback(
    (event: MouseEvent | ReactMouseEvent<Element, MouseEvent>, node: NodeType) => {
      event.preventDefault();

      const screenPosition = { x: event.clientX, y: event.clientY };
      const flowPosition =
        reactFlowInstance?.screenToFlowPosition(screenPosition) || screenPosition;

      setContextMenuOpen(true, screenPosition, flowPosition, node.id);
    },
    [reactFlowInstance, setContextMenuOpen],
  );

  const onClick = useCallback(() => {
    if (contextMenuOpen) {
      setContextMenuOpen(false);
    }
  }, [contextMenuOpen, setContextMenuOpen]);

  const onApply = useCallback(async () => {
    setApplyStatus("Applying...");
    const graph: AudioGraph = {
      nodes: nodes.map(serializeNode),
      edges: edges.map((edge) => ({
        id: edge.id,
        from: edge.source,
        to: edge.target,
        toHandle: edge.targetHandle ?? undefined,
        frequency: edge.data?.frequency,
        channels: edge.data?.channels,
        bitsPerSample: edge.data?.bitsPerSample,
      })),
    };

    console.log("Applying graph:", graph);

    try {
      await invoke("setup_runtime", {
        graph,
        host: selectedAudioHost,
        bufferSize: 512,
      });
      startRenderPolling();
      setIsRuntimeEnabled(true);
      setApplyStatus("Applied successfully");
      setTimeout(() => setApplyStatus(null), 3000);
    } catch (e: any) {
      console.error("setup_runtime failed:", e);
      setApplyStatus(`Error: ${e}`);
    }
  }, [nodes, edges, selectedAudioHost]);

  const onSave = useCallback(async () => {
    const file: CableGraphFile = { version: 1, nodes, edges };
    await invoke("save_graph", { content: JSON.stringify(file, null, 2) });
  }, [nodes, edges]);

  useEffect(() => {
    document.title = "Cable";
    initializeApp();
  }, [initializeApp]);

  useEffect(() => {
    let unlisten: (() => void) | null = null;

    getCurrentWebview()
      .onDragDropEvent(async (event) => {
        if (event.payload.type !== "drop") return;
        const jsonPath = event.payload.paths.find((p) => p.endsWith(".json"));
        if (!jsonPath) return;

        try {
          const content = await invoke("read_text_file", { path: jsonPath });
          const parsed = JSON.parse(content) as CableGraphFile;
          if (
            parsed.version !== 1 ||
            !Array.isArray(parsed.nodes) ||
            !Array.isArray(parsed.edges)
          ) {
            setApplyStatus("Error: 올바르지 않은 그래프 파일 형식입니다.");
            return;
          }
          await invoke("disable_runtime");
          stopRenderPolling();
          setIsRuntimeEnabled(false);
          loadGraph(parsed.nodes, parsed.edges);
          setApplyStatus("그래프를 불러왔습니다. Apply를 눌러 적용하세요.");
          setTimeout(() => setApplyStatus(null), 4000);
        } catch {
          setApplyStatus("Error: 파일을 불러올 수 없습니다.");
        }
      })
      .then((fn) => {
        unlisten = fn;
      });

    return () => {
      unlisten?.();
    };
  }, [loadGraph, stopRenderPolling]);

  return (
    <div className="h-full w-full">
      <ReactFlow
        nodes={nodes}
        edges={edges}
        nodeTypes={nodeTypes}
        onNodesChange={onNodesChange}
        onEdgesChange={onEdgesChange}
        onConnect={onConnect}
        onInit={setReactFlowInstance}
        fitView
        onPaneContextMenu={onPaneContextMenu}
        onNodeContextMenu={onNodeContextMenu}
        onClick={onClick}
      >
        <Background color="black" variant={BackgroundVariant.Dots} />
      </ReactFlow>
      <Menu />
      <ContextMenu />
      <Panel position="bottom-center">
        <div className="flex flex-col items-center gap-2 mb-2">
          {applyStatus && (
            <div
              className={`text-xs px-3 py-1 rounded-full ${
                applyStatus.startsWith("Error")
                  ? "bg-red-900/80 text-red-200"
                  : applyStatus === "Applying..."
                    ? "bg-yellow-900/80 text-yellow-200"
                    : "bg-green-900/80 text-green-200"
              }`}
            >
              {applyStatus}
            </div>
          )}
          <div className="flex items-center gap-1 bg-gray-900/90 backdrop-blur-sm border border-gray-700 rounded-2xl px-3 py-2 shadow-xl">
            {/* Driver status */}
            <div
              className={`flex items-center gap-1.5 px-2 text-xs font-medium ${
                driverConnected ? "text-green-400" : "text-red-400"
              }`}
              title={driverConnected ? "Driver connected" : "Driver offline"}
            >
              <Cpu size={15} />
              <span>{driverConnected ? "Online" : "Offline"}</span>
            </div>

            <div className="w-px h-5 bg-gray-700 mx-1" />

            {/* Save */}
            <button
              className="p-2 rounded-xl text-gray-400 hover:text-white hover:bg-gray-700 transition-colors"
              onClick={onSave}
              title="Save graph"
            >
              <Save size={16} />
            </button>

            {/* Apply */}
            <button
              className="p-2 rounded-xl text-gray-400 hover:text-white hover:bg-gray-700 transition-colors"
              onClick={onApply}
              title="Apply graph"
            >
              <Zap size={16} />
            </button>

            <div className="w-px h-5 bg-gray-700 mx-1" />

            {/* Enable / Disable Runtime */}
            <button
              className={`p-2 rounded-xl transition-colors ${
                isRuntimeEnabled
                  ? "text-green-400 hover:text-white hover:bg-gray-700"
                  : "text-gray-400 hover:text-white hover:bg-gray-700"
              }`}
              onClick={async () => {
                if (isRuntimeEnabled) {
                  await invoke("disable_runtime");
                  stopRenderPolling();
                  setIsRuntimeEnabled(false);
                } else {
                  await invoke("enable_runtime");
                  startRenderPolling();
                  setIsRuntimeEnabled(true);
                }
              }}
              title={isRuntimeEnabled ? "Disable runtime" : "Enable runtime"}
            >
              {isRuntimeEnabled ? <Square size={16} /> : <Play size={16} />}
            </button>
          </div>
        </div>
      </Panel>
    </div>
  );
}

export default App;
