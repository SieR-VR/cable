import { MouseEvent, useCallback, useEffect, useState } from "react";
import { ReactFlow, Background, BackgroundVariant, Panel } from "@xyflow/react";

import "@xyflow/react/dist/style.css";

import Menu from "./components/Menu";
import { useAppStore } from "./state";
import { ContextMenu } from "./components/ContextMenu";
import { AudioGraph, nodeTypes } from "./types";
import { invoke } from "@tauri-apps/api/core";

function App() {
  const {
    contextMenuOpen,
    setContextMenuOpen,
    initializeApp,
    selectedAudioHost,
    nodes,
    edges,
    onNodesChange,
    onEdgesChange,
    onConnect,
  } = useAppStore();

  const [isRuntimeEnabled, setIsRuntimeEnabled] = useState(false);

  const onContextMenu = useCallback((event: MouseEvent) => {
    event.preventDefault();

    setContextMenuOpen(true, { x: event.clientX, y: event.clientY });
  }, []);

  const onClick = useCallback(() => {
    if (contextMenuOpen) {
      setContextMenuOpen(false);
    }
  }, [contextMenuOpen]);

  const onApply = useCallback(() => {
    const graph: AudioGraph = {
      nodes: nodes.map((node) => ({
        type: node.type,
        data: {
          id: node.id,
          device: node.data.device,
        },
      })),
      edges: edges.map((edge) => ({
        id: edge.id,
        from: edge.source,
        to: edge.target,
        frequency: edge.data?.frequency,
        channels: edge.data?.channels,
        bitsPerSample: edge.data?.bitsPerSample,
      })),
    };

    console.log(graph);

    invoke("setup_runtime", { graph, host: selectedAudioHost, bufferSize: 512 });
  }, [nodes, edges]);

  useEffect(() => {
    document.title = "Cable";
    initializeApp();
  }, []);

  return (
    <div className="h-full w-full">
      <ReactFlow
        nodes={nodes}
        edges={edges}
        nodeTypes={nodeTypes}
        onNodesChange={onNodesChange}
        onEdgesChange={onEdgesChange}
        onConnect={onConnect}
        fitView
        onContextMenu={onContextMenu}
        onClick={onClick}
      >
        <Background color="black" variant={BackgroundVariant.Dots} />
      </ReactFlow>
      <Menu />
      <ContextMenu />
      <Panel position="bottom-center">
        <div className="text-sm text-gray-500">
          <button
            className="bg-gray-700 text-white px-2 py-1 rounded"
            onClick={onApply}
          >
            Apply
          </button>
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
