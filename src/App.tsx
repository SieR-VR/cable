import { invoke } from "@tauri-apps/api/core";
import { ReactFlow, Background, BackgroundVariant, Panel, ReactFlowInstance } from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import { DragEvent, MouseEvent as ReactMouseEvent, useCallback, useEffect, useState } from "react";

import { ContextMenu } from "./components/ContextMenu";
import Menu from "./components/Menu";

import { useAppStore } from "./state";
import { AudioGraph, CableGraphFile, EdgeType, NodeType, nodeTypes } from "./types";

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
      nodes: nodes.map((node) => {
        if (node.type === "virtualAudioInput" || node.type === "virtualAudioOutput") {
          return {
            type: node.type,
            data: {
              id: node.id,
              deviceId: (node.data as any).deviceId || "",
              name: (node.data as any).name || "",
            },
          };
        }
        if (node.type === "spectrumAnalyzer") {
          return {
            type: node.type,
            data: {
              id: node.id,
              fftSize: (node.data as any).fftSize ?? 1024,
            },
          };
        }
        if (node.type === "waveformMonitor") {
          return {
            type: node.type,
            data: {
              id: node.id,
              windowSize: (node.data as any).windowSize ?? 2048,
            },
          };
        }
        if (node.type === "appAudioCapture") {
          return {
            type: node.type,
            data: {
              id: node.id,
              processId: (node.data as any).processId ?? 0,
              windowTitle: (node.data as any).windowTitle ?? "",
            },
          };
        }
        return {
          type: node.type,
          data: {
            id: node.id,
            device: (node.data as any).device,
          },
        };
      }),
      edges: edges.map((edge) => ({
        id: edge.id,
        from: edge.source,
        to: edge.target,
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

  const onSave = useCallback(() => {
    const file: CableGraphFile = { version: 1, nodes, edges };
    const blob = new Blob([JSON.stringify(file, null, 2)], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = "cable-graph.json";
    a.click();
    URL.revokeObjectURL(url);
  }, [nodes, edges]);

  const onDragOver = useCallback((event: DragEvent<HTMLDivElement>) => {
    event.preventDefault();
    event.dataTransfer.dropEffect = "copy";
  }, []);

  const onDrop = useCallback(
    (event: DragEvent<HTMLDivElement>) => {
      event.preventDefault();
      const file = event.dataTransfer.files[0];
      if (!file) return;

      const reader = new FileReader();
      reader.onload = async (e) => {
        try {
          const parsed = JSON.parse(e.target?.result as string) as CableGraphFile;
          if (
            parsed.version !== 1 ||
            !Array.isArray(parsed.nodes) ||
            !Array.isArray(parsed.edges)
          ) {
            setApplyStatus("Error: 올바르지 않은 그래프 파일 형식입니다.");
            return;
          }
          if (isRuntimeEnabled) {
            await invoke("disable_runtime");
            stopRenderPolling();
            setIsRuntimeEnabled(false);
          }
          loadGraph(parsed.nodes, parsed.edges);
          setApplyStatus("그래프를 불러왔습니다. Apply를 눌러 적용하세요.");
          setTimeout(() => setApplyStatus(null), 4000);
        } catch {
          setApplyStatus("Error: JSON 파싱 실패");
        }
      };
      reader.readAsText(file);
    },
    [isRuntimeEnabled, loadGraph, stopRenderPolling],
  );

  useEffect(() => {
    document.title = "Cable";
    initializeApp();
  }, [initializeApp]);

  return (
    <div className="h-full w-full" onDragOver={onDragOver} onDrop={onDrop}>
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
        <div className="flex flex-col items-center gap-1">
          {applyStatus && (
            <div
              className={`text-xs px-2 py-1 rounded ${applyStatus.startsWith("Error") ? "bg-red-800 text-red-200" : applyStatus === "Applying..." ? "bg-yellow-800 text-yellow-200" : "bg-green-800 text-green-200"}`}
            >
              {applyStatus}
            </div>
          )}
          <div className="flex gap-2 items-center text-sm text-gray-500">
            <span
              className={`inline-block w-2 h-2 rounded-full ${driverConnected ? "bg-green-400" : "bg-red-400"}`}
            />
            <span>{driverConnected ? "Driver connected" : "Driver offline"}</span>
            <button className="bg-gray-700 text-white px-2 py-1 rounded" onClick={onApply}>
              Apply
            </button>
          </div>
        </div>
      </Panel>
      <Panel position="top-right">
        <div className="text-sm text-gray-500 flex gap-2">
          <button className="bg-gray-700 text-white px-2 py-1 rounded" onClick={onSave}>
            Save
          </button>
          <button
            className="bg-gray-700 text-white px-2 py-1 rounded"
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
          >
            {isRuntimeEnabled ? "Disable" : "Enable"} Runtime
          </button>
        </div>
      </Panel>
    </div>
  );
}

export default App;
