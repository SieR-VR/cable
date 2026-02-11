import { MouseEvent, useCallback, useEffect } from "react";
import { ReactFlow, Background, BackgroundVariant } from "@xyflow/react";

import "@xyflow/react/dist/style.css";

import Menu from "./components/Menu";
import { useAppStore } from "./state";
import { ContextMenu } from "./components/ContextMenu";
import { nodeTypes } from "./types";

function App() {
  const {
    contextMenuOpen,
    setContextMenuOpen,
    initializeApp,
    nodes,
    edges,
    onNodesChange,
    onEdgesChange,
    onConnect,
  } = useAppStore();

  const onContextMenu = useCallback((event: MouseEvent) => {
    event.preventDefault();

    setContextMenuOpen(true, { x: event.clientX, y: event.clientY });
  }, []);

  const onClick = useCallback(() => {
    if (contextMenuOpen) {
      setContextMenuOpen(false);
    }
  }, [contextMenuOpen]);

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
    </div>
  );
}

export default App;
