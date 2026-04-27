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
import { createWithEqualityFn } from "zustand/traditional";

import { AudioDevice, EdgeType, NodeRenderData, NodeType, VirtualDevice, VstPluginInfo } from "./types";

/** Module-level interval ID for the global render polling loop. */
let renderPollIntervalId: ReturnType<typeof setInterval> | null = null;

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

  /** Virtual devices created via the driver (managed in the menu panel). */
  virtualDevices: VirtualDevice[];

  /** Cached VST3 plugin list from the last scan. */
  vstPluginList: VstPluginInfo[];

  /** Latest per-frame render data for all active visualizer nodes. */
  nodeRenderData: Record<string, NodeRenderData>;

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
      | "virtualAudioOutput"
      | "spectrumAnalyzer"
      | "waveformMonitor"
      | "appAudioCapture"
      | "mixer"
      | "vst",
  ) => void;
  removeNodeAtContextMenu: () => void;

  setSelectedAudioHost: (host: string) => void;
  setDriverConnected: (connected: boolean) => void;

  initializeApp: () => Promise<void>;

  // Virtual device management
  addVirtualDevice: (name: string, deviceType: "render" | "capture") => Promise<void>;
  removeVirtualDevice: (deviceId: string) => Promise<void>;
  renameVirtualDevice: (deviceId: string, newName: string) => Promise<void>;

  onNodesChange: (changes: NodeChange<NodeType>[]) => void;
  onEdgesChange: (changes: EdgeChange<EdgeType>[]) => void;
  onConnect: (connection: Connection) => void;
  updateNode: (id: string, data: any) => void;

  /** Replace the entire graph with loaded nodes and edges. */
  loadGraph: (nodes: NodeType[], edges: EdgeType[]) => void;

  /** Scan the system for VST3 plugins and cache the result. */
  scanVstPlugins: () => Promise<void>;

  /** Start the single global 30fps polling loop for visualizer render data. */
  startRenderPolling: () => void;
  /** Stop the polling loop and clear cached render data. */
  stopRenderPolling: () => void;
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
  virtualDevices: [],
  vstPluginList: [],
  nodeRenderData: {},

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

    const isVirtual = type === "virtualAudioInput" || type === "virtualAudioOutput";
    const isSpectrumAnalyzer = type === "spectrumAnalyzer";
    const isWaveformMonitor = type === "waveformMonitor";
    const isAppAudioCapture = type === "appAudioCapture";
    const isMixer = type === "mixer";
    const isVst = type === "vst";

    const data = isVirtual
      ? { deviceId: "", name: "", edgeType: null }
      : isSpectrumAnalyzer
        ? { fftSize: 1024, edgeType: null }
        : isWaveformMonitor
          ? { windowSize: 2048, edgeType: null }
          : isAppAudioCapture
            ? { processId: null, windowTitle: null, edgeType: null }
            : isMixer
              ? { edgeType: null }
              : isVst
                ? { pluginPath: "", numInputs: 1, numOutputs: 1, channels: 2, params: [] }
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
          edge.source !== contextMenuTargetNodeId && edge.target !== contextMenuTargetNodeId,
      ),
    });
  },

  setSelectedAudioHost: (host: string) => set({ selectedAudioHost: host }),
  setDriverConnected: (connected: boolean) => set({ driverConnected: connected }),

  initializeApp: async () => {
    async function initHosts() {
      const hosts = await invoke<string[]>("get_audio_hosts");
      set({ availableAudioHosts: hosts, selectedAudioHost: hosts[0] || null });
      return hosts[0] || null;
    }

    async function initDevices(host: string | null) {
      if (!host) {
        set({ availableAudioInputDevices: null });
        return;
      }

      const [inputDevices, outputDevices] = await invoke<[AudioDevice[], AudioDevice[]]>(
        "get_audio_devices",
        { host },
      );
      set({
        availableAudioInputDevices: inputDevices,
        availableAudioOutputDevices: outputDevices,
      });
    }

    async function initDriver() {
      try {
        const connected = await invoke<boolean>("connect_driver");
        set({ driverConnected: connected });

        if (connected) {
          const devices = await invoke<VirtualDevice[]>("list_virtual_devices");
          set({ virtualDevices: devices });
        }
      } catch (e) {
        console.warn("Failed to connect to CableAudio driver:", e);
        set({ driverConnected: false });
      }
    }

    let host: string | null = null;
    try {
      host = await initHosts();
    } catch (e) {
      console.warn("Failed to initialize audio hosts:", e);
    }
    await Promise.all([
      initDevices(host).catch((e) => console.warn("Failed to initialize audio devices:", e)),
      initDriver(),
    ]);
  },

  addVirtualDevice: async (name, deviceType) => {
    try {
      const device = await invoke<VirtualDevice>("create_virtual_device", {
        name,
        deviceType,
      });
      set({ virtualDevices: [...get().virtualDevices, device] });
    } catch (e) {
      console.error("Failed to create virtual device:", e);
      throw e;
    }
  },

  removeVirtualDevice: async (deviceId) => {
    try {
      await invoke("remove_virtual_device", { deviceId });
      set({
        virtualDevices: get().virtualDevices.filter((d) => d.id !== deviceId),
      });
    } catch (e) {
      console.error("Failed to remove virtual device:", e);
      throw e;
    }
  },

  renameVirtualDevice: async (deviceId, newName) => {
    try {
      await invoke("rename_virtual_device", { deviceId, newName });
      set({
        virtualDevices: get().virtualDevices.map((d) =>
          d.id === deviceId ? { ...d, name: newName } : d,
        ),
      });
    } catch (e) {
      console.error("Failed to rename virtual device:", e);
      throw e;
    }
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
    const { nodes, edges } = get();

    const sourceNode = nodes.find((node) => node.id === connection.source);
    const targetNode = nodes.find((node) => node.id === connection.target);

    const fromType =
      sourceNode?.data && "edgeType" in sourceNode.data ? sourceNode.data.edgeType : null;
    const toType =
      targetNode?.data && "edgeType" in targetNode.data ? targetNode.data.edgeType : null;

    if (fromType && toType && fromType !== toType) {
      console.warn(`Cannot connect nodes with different audio formats: ${fromType} -> ${toType}`);
      return;
    }

    // Only one edge per input handle is allowed.
    const isDuplicateInput = edges.some(
      (edge) =>
        edge.target === connection.target &&
        edge.targetHandle === connection.targetHandle,
    );
    if (isDuplicateInput) {
      console.warn(
        `Input handle already occupied: ${connection.target}:${connection.targetHandle}`,
      );
      return;
    }

    set({
      edges: addEdge(connection, edges),
    });
  },

  updateNode: (id: string, data: any) =>
    set({
      nodes: get().nodes.map((node) =>
        node.id === id ? { ...node, data: { ...node.data, ...data } } : node,
      ),
    }),

  loadGraph: (nodes: NodeType[], edges: EdgeType[]) => set({ nodes, edges }),

  scanVstPlugins: async () => {
    const plugins = await invoke("scan_vst3_plugins");
    set({ vstPluginList: plugins });
  },

  startRenderPolling:() => {
    if (renderPollIntervalId !== null) clearInterval(renderPollIntervalId);
    renderPollIntervalId = setInterval(async () => {
      try {
        const data = await invoke("get_node_render_data");
        set({ nodeRenderData: data });
      } catch {
        // Runtime may not be initialized yet; ignore.
      }
    }, 33);
  },

  stopRenderPolling: () => {
    if (renderPollIntervalId !== null) {
      clearInterval(renderPollIntervalId);
      renderPollIntervalId = null;
    }
    set({ nodeRenderData: {} });
  },
}));
