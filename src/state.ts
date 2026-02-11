import { createWithEqualityFn } from "zustand/traditional";
import { invoke } from "@tauri-apps/api/core";
import {
  addEdge,
  applyEdgeChanges,
  applyNodeChanges,
  Connection,
  Edge,
  EdgeChange,
  Node,
  NodeChange,
  XYPosition,
} from "@xyflow/react";

import { AudioDevice } from "./types";

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

export interface AppState {
  menuOpen: boolean;

  contextMenuOpen: boolean;
  contextMenuPosition: XYPosition;

  availableAudioHosts: string[] | null;
  selectedAudioHost: string | null;

  availableAudioInputDevices: AudioDevice[] | null;
  availableAudioOutputDevices: AudioDevice[] | null;

  nodes: Node[];
  edges: Edge[];

  setMenuOpen: (open: boolean) => void;

  setContextMenuOpen: (open: boolean, position?: XYPosition) => void;

  setSelectedAudioHost: (host: string) => void;

  initializeApp: () => Promise<void>;

  onNodesChange: (changes: NodeChange[]) => void;
  onEdgesChange: (changes: EdgeChange[]) => void;
  onConnect: (connection: Connection) => void;
  updateNode: (id: string, data: any) => void;
}

export const useAppStore = createWithEqualityFn<AppState>((set, get) => ({
  menuOpen: false,

  contextMenuOpen: false,
  contextMenuPosition: { x: 0, y: 0 },

  availableAudioHosts: null,
  selectedAudioHost: null,

  availableAudioInputDevices: null,
  availableAudioOutputDevices: null,

  nodes: initialNodes,
  edges: [],

  setMenuOpen: (open: boolean) => set({ menuOpen: open }),

  setContextMenuOpen: (open: boolean, position: XYPosition = { x: 0, y: 0 }) =>
    set({ contextMenuOpen: open, contextMenuPosition: position }),

  setSelectedAudioHost: (host: string) => set({ selectedAudioHost: host }),

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

    const host = await initHosts();
    await initDevices(host);
  },

  onNodesChange: (changes) => {
    set({
      nodes: applyNodeChanges(changes, get().nodes),
    });
  },

  onEdgesChange: (changes) => {
    set({
      edges: applyEdgeChanges(changes, get().edges),
    });
  },

  onConnect: (connection) => {
    const nodes = get().nodes;

    const fromType = nodes.find((node) => node.id === connection.source)?.data
      .edgeType;
    const toType = nodes.find((node) => node.id === connection.target)?.data
      .edgeType;

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
