import { createWithEqualityFn } from "zustand/traditional";
import { invoke } from "@tauri-apps/api/core";
import {
  addEdge,
  applyEdgeChanges,
  applyNodeChanges,
  Connection,
  EdgeChange,
  NodeChange,
  XYPosition,
} from "@xyflow/react";

import { AudioDevice, EdgeType, NodeType } from "./types";

const initialNodes: NodeType[] = [
  {
    id: "node-1",
    type: "audioInputDevice",
    dragHandle: ".drag-handle__custom",
    position: { x: 100, y: 0 },
    data: {
      device: null,
      edgeType: null,
    },
  },
  {
    id: "node-2",
    type: "audioOutputDevice",
    dragHandle: ".drag-handle__custom",
    position: { x: 500, y: 0 },
    data: {
      device: null,
      edgeType: null,
    },
  },
];

export interface AppState {
  menuOpen: boolean;

  contextMenuOpen: boolean;
  contextMenuPosition: XYPosition;
  contextMenuFlowPosition: XYPosition;
  contextMenuTargetNodeId: string | null;

  availableAudioHosts: string[] | null;
  selectedAudioHost: string | null;

  availableAudioInputDevices: AudioDevice[] | null;
  availableAudioOutputDevices: AudioDevice[] | null;

  driverConnected: boolean;

  nodes: NodeType[];
  edges: EdgeType[];

  setMenuOpen: (open: boolean) => void;

  setContextMenuOpen: (
    open: boolean,
    position?: XYPosition,
    flowPosition?: XYPosition,
    targetNodeId?: string | null,
  ) => void;

  addNodeAtContextMenu: (
    type:
      | "audioInputDevice"
      | "audioOutputDevice"
      | "virtualAudioInput"
      | "virtualAudioOutput",
  ) => void;
  removeNodeAtContextMenu: () => void;

  setSelectedAudioHost: (host: string) => void;
  setDriverConnected: (connected: boolean) => void;

  initializeApp: () => Promise<void>;

  onNodesChange: (changes: NodeChange<NodeType>[]) => void;
  onEdgesChange: (changes: EdgeChange<EdgeType>[]) => void;
  onConnect: (connection: Connection) => void;
  updateNode: (id: string, data: any) => void;
}

export const useAppStore = createWithEqualityFn<AppState>((set, get) => ({
  menuOpen: false,

  contextMenuOpen: false,
  contextMenuPosition: { x: 0, y: 0 },
  contextMenuFlowPosition: { x: 0, y: 0 },
  contextMenuTargetNodeId: null,

  availableAudioHosts: null,
  selectedAudioHost: null,

  availableAudioInputDevices: null,
  availableAudioOutputDevices: null,

  driverConnected: false,

  nodes: initialNodes,
  edges: [],

  setMenuOpen: (open: boolean) => set({ menuOpen: open }),

  setContextMenuOpen: (
    open: boolean,
    position: XYPosition = { x: 0, y: 0 },
    flowPosition: XYPosition = { x: 0, y: 0 },
    targetNodeId: string | null = null,
  ) =>
    set({
      contextMenuOpen: open,
      contextMenuPosition: position,
      contextMenuFlowPosition: flowPosition,
      contextMenuTargetNodeId: targetNodeId,
    }),

  addNodeAtContextMenu: (type) => {
    const { nodes, contextMenuFlowPosition } = get();

    const usedIds = new Set(nodes.map((node) => node.id));
    let nextId = nodes.length + 1;
    while (usedIds.has(`node-${nextId}`)) {
      nextId += 1;
    }

    const isVirtual =
      type === "virtualAudioInput" || type === "virtualAudioOutput";

    const data = isVirtual
      ? { name: "", edgeType: null }
      : { device: null, edgeType: null };

    const newNode: NodeType = {
      id: `node-${nextId}`,
      type,
      dragHandle: ".drag-handle__custom",
      position: contextMenuFlowPosition,
      data,
    } as NodeType;

    set({ nodes: [...nodes, newNode] });
  },

  removeNodeAtContextMenu: () => {
    const { nodes, edges, contextMenuTargetNodeId } = get();

    if (!contextMenuTargetNodeId) {
      return;
    }

    set({
      nodes: nodes.filter((node) => node.id !== contextMenuTargetNodeId),
      edges: edges.filter(
        (edge) =>
          edge.source !== contextMenuTargetNodeId &&
          edge.target !== contextMenuTargetNodeId,
      ),
    });
  },

  setSelectedAudioHost: (host: string) => set({ selectedAudioHost: host }),
  setDriverConnected: (connected: boolean) => set({ driverConnected: connected }),

  initializeApp: async () => {
    async function initHosts() {
      const hosts = await invoke("get_audio_hosts");
      set({ availableAudioHosts: hosts, selectedAudioHost: hosts[0] || null });

      return hosts[0] || null;
    }

    async function initDevices(host: string | null) {
      if (!host) {
        set({ availableAudioInputDevices: null });
        return;
      }

      const [inputDevices, outputDevices] = await invoke("get_audio_devices", {
        host,
      });
      set({
        availableAudioInputDevices: inputDevices,
        availableAudioOutputDevices: outputDevices,
      });
    }

    async function initDriver() {
      try {
        const connected = await invoke("connect_driver");
        set({ driverConnected: connected });
      } catch (e) {
        console.warn("Failed to connect to CableAudio driver:", e);
        set({ driverConnected: false });
      }
    }

    const host = await initHosts();
    await Promise.all([initDevices(host), initDriver()]);
  },

  onNodesChange: (changes) => {
    set({
      nodes: applyNodeChanges<NodeType>(changes, get().nodes),
    });
  },

  onEdgesChange: (changes) => {
    set({
      edges: applyEdgeChanges<EdgeType>(changes, get().edges),
    });
  },

  onConnect: (connection) => {
    const nodes = get().nodes;

    const sourceNode = nodes.find((node) => node.id === connection.source);
    const targetNode = nodes.find((node) => node.id === connection.target);

    const fromType =
      sourceNode?.data && "edgeType" in sourceNode.data
        ? sourceNode.data.edgeType
        : null;
    const toType =
      targetNode?.data && "edgeType" in targetNode.data
        ? targetNode.data.edgeType
        : null;

    if (fromType && toType && fromType !== toType) {
      console.warn(
        `Cannot connect nodes with different audio formats: ${fromType} -> ${toType}`,
      );
      return;
    }

    set({
      edges: addEdge(connection, get().edges),
    });
  },

  updateNode: (id: string, data: any) =>
    set({
      nodes: get().nodes.map((node) =>
        node.id === id ? { ...node, data: { ...node.data, ...data } } : node,
      ),
    }),
}));
