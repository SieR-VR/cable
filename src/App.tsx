import { useCallback, useState } from "react";
import {
  ReactFlow,
  applyEdgeChanges,
  applyNodeChanges,
  addEdge,
  Background,
  BackgroundVariant,
  MiniMap,
  Position,
  Panel,
} from "@xyflow/react";

import { invoke } from "@tauri-apps/api/core";

import "@xyflow/react/dist/style.css";

const initialNodes = [
  {
    id: "n1",
    position: { x: 0, y: 0 },
    data: { label: "Node 1" },
    sourcePosition: Position.Right,
    targetPosition: Position.Left,
  },
  {
    id: "n2",
    position: { x: 0, y: 100 },
    data: { label: "Node 2" },
    sourcePosition: Position.Right,
    targetPosition: Position.Left,
  },
];
const initialEdges = [{ id: "n1-n2", source: "n1", target: "n2" }];

function App() {
  const [nodes, setNodes] = useState(initialNodes);
  const [edges, setEdges] = useState(initialEdges);

  const onNodesChange = useCallback(
    (changes) =>
      setNodes((nodesSnapshot) => applyNodeChanges(changes, nodesSnapshot)),
    [],
  );
  const onEdgesChange = useCallback(
    (changes) =>
      setEdges((edgesSnapshot) => applyEdgeChanges(changes, edgesSnapshot)),
    [],
  );
  const onConnect = useCallback(
    (params) => setEdges((edgesSnapshot) => addEdge(params, edgesSnapshot)),
    [],
  );

  return (
    <ReactFlow
      nodes={nodes}
      edges={edges}
      onNodesChange={onNodesChange}
      onEdgesChange={onEdgesChange}
      onConnect={onConnect}
      fitView
    >
      <Background color="black" variant={BackgroundVariant.Dots} />
      <MiniMap nodeColor={() => "blue"} nodeStrokeWidth={3} zoomable pannable />
      <Panel position="bottom-center">
        <button
          onClick={() => {
            console.log("fetching audio devices...");
            invoke("get_audio_devices").then((devices) => console.log(devices));
          }}
          className="border left-5 top-5 z-10"
        >
          Get Audio Devices
        </button>
      </Panel>
    </ReactFlow>
  );
}

export default App;
