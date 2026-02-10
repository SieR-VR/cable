import { MouseEvent, useCallback, useEffect } from "react";
import {
  ReactFlow,
  addEdge,
  Background,
  BackgroundVariant,
  Connection,
  useNodesState,
  useEdgesState,
} from "@xyflow/react";

import "@xyflow/react/dist/style.css";

import Menu from "./components/Menu";
import { initializeApp, setContextMenuOpen, useAppState } from "./state";
import { ContextMenu } from "./components/ContextMenu";
import { nodeTypes } from "./types";

const initialNodes = [
  {
    id: "node-1",
    type: "audioInputDevice",
    dragHandle: ".drag-handle__custom",
    position: { x: 100, y: 0 },
    data: {},
  },
  {
    id: "node-2",
    type: "audioOutputDevice",
    dragHandle: ".drag-handle__custom",
    position: { x: 500, y: 0 },
    data: {},
  },
];

function App() {
  const { contextMenuOpen } = useAppState();

  const [nodes, setNodes, onNodesChange] = useNodesState(initialNodes);
  const [edges, setEdges, onEdgesChange] = useEdgesState([]);

  const onConnect = useCallback(
    (connection: Connection) =>
      setEdges((edgesSnapshot) => addEdge(connection, edgesSnapshot)),
    [],
  );

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
