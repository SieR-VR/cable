import { invoke } from "@tauri-apps/api/core";
import {
  ReactFlow,
  Background,
  BackgroundVariant,
  Panel,
  ReactFlowInstance,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import {
  MouseEvent as ReactMouseEvent,
  useCallback,
  useEffect,
  useState,
} from "react";

import { ContextMenu } from "./components/ContextMenu";
import Menu from "./components/Menu";

import { useAppStore } from "./state";
import { AudioGraph, EdgeType, NodeType, nodeTypes } from "./types";

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
        reactFlowInstance?.screenToFlowPosition(screenPosition) ||
        screenPosition;

      setContextMenuOpen(true, screenPosition, flowPosition);
    },
    [reactFlowInstance, setContextMenuOpen],
  );

  const onNodeContextMenu = useCallback(
    (
      event: MouseEvent | ReactMouseEvent<Element, MouseEvent>,
      node: NodeType,
    ) => {
      event.preventDefault();

      const screenPosition = { x: event.clientX, y: event.clientY };
      const flowPosition =
        reactFlowInstance?.screenToFlowPosition(screenPosition) ||
        screenPosition;

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
        if (
          node.type === "virtualAudioInput" ||
          node.type === "virtualAudioOutput"
        ) {
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
        buffer_size: 512,
      });
      setIsRuntimeEnabled(true);
      setApplyStatus("Applied successfully");
      setTimeout(() => setApplyStatus(null), 3000);
    } catch (e: any) {
      console.error("setup_runtime failed:", e);
      setApplyStatus(`Error: ${e}`);
    }
  }, [nodes, edges, selectedAudioHost]);

  useEffect(() => {
    document.title = "Cable";
    initializeApp();
  }, [initializeApp]);

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
            <span>
              {driverConnected ? "Driver connected" : "Driver offline"}
            </span>
            <button
              className="bg-gray-700 text-white px-2 py-1 rounded"
              onClick={onApply}
            >
              Apply
            </button>
          </div>
        </div>
      </Panel>
      <Panel position="top-right">
        <div className="text-sm text-gray-500">
          <button
            className="bg-gray-700 text-white px-2 py-1 rounded"
            onClick={async () => {
              if (isRuntimeEnabled) {
                await invoke("disable_runtime");
                setIsRuntimeEnabled(false);
              } else {
                await invoke("enable_runtime");
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
