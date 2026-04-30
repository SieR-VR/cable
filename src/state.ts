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

import { ValidationResult } from "./graph/edge-type";
import { runCascade, runFullValidation, ValidationContext } from "./graph/validation";
import { persistSetting, readSetting, SETTING_KEYS } from "./settings";
import { AudioDevice, BluetoothBatteryInfo, EdgeType, NodeRenderData, NodeType, VirtualDevice, VstPluginInfo, nodeDefs, serializeEdge, serializeNode } from "./types";

export const BUFFER_SIZE_OPTIONS = [64, 128, 256, 512, 1024, 2048] as const;
export const DEFAULT_BUFFER_SIZE = 512;

const fireAndForget = (p: Promise<unknown>, label: string) => {
  p.catch((e) => console.warn(`${label} failed:`, e));
};

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

  /** Audio buffer size in frames. Persisted in settings.json. */
  bufferSize: number;

  availableAudioInputDevices: AudioDevice[] | null;
  availableAudioOutputDevices: AudioDevice[] | null;

  driverConnected: boolean;

  /** Virtual devices created via the driver (managed in the menu panel). */
  virtualDevices: VirtualDevice[];

  /** Cached VST3 plugin list from the last scan. */
  vstPluginList: VstPluginInfo[];

  /** Latest per-frame render data for all active visualizer nodes. */
  nodeRenderData: Record<string, NodeRenderData>;

  /** Live AirPods battery levels keyed by audio container_id. Transient. */
  bluetoothBattery: Record<string, BluetoothBatteryInfo>;
  /** Whether the BLE advertisement watcher should run. Persisted. */
  bluetoothBatteryEnabled: boolean;

  /** Whether closing the window minimizes to tray. Persisted. */
  minimizeToTrayEnabled: boolean;

  /** Latest per-node validation result, keyed by node id. */
  validation: Record<string, ValidationResult>;

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
      | "gain"
      | "channelMerge"
      | "channelSplit"
      | "delay"
      | "compressor"
      | "reverb"
      | "echo"
      | "vst",
  ) => void;
  removeNodeAtContextMenu: () => void;

  setSelectedAudioHost: (host: string) => void;
  setBufferSize: (size: number) => void;
  setDriverConnected: (connected: boolean) => void;

  setBluetoothBatteryEnabled: (enabled: boolean) => Promise<void>;
  setBluetoothBattery: (info: BluetoothBatteryInfo) => void;
  setMinimizeToTrayEnabled: (enabled: boolean) => Promise<void>;

  initializeApp: () => Promise<void>;

  // Virtual device management
  addVirtualDevice: (name: string, deviceType: "render" | "capture") => Promise<void>;
  removeVirtualDevice: (deviceId: string) => Promise<void>;
  renameVirtualDevice: (deviceId: string, newName: string) => Promise<void>;
  /**
   * Update the format preset for a virtual device and propagate the change to
   * any graph nodes that reference that device, then re-validate.
   */
  setVirtualDeviceFormat: (
    deviceId: string,
    channels: number,
    sampleRate: number,
    bitsPerSample: number,
  ) => Promise<void>;

  onNodesChange: (changes: NodeChange<NodeType>[]) => void;
  onEdgesChange: (changes: EdgeChange<EdgeType>[]) => void;
  onConnect: (connection: Connection) => void;
  updateNode: (id: string, data: any) => void;

  /** Replace the entire graph with loaded nodes and edges. */
  loadGraph: (nodes: NodeType[], edges: EdgeType[]) => void;

  /** Force a full type-check pass over the entire graph. */
  runFullTypeCheck: () => void;

  /** Scan the system for VST3 plugins and cache the result. */
  scanVstPlugins: () => Promise<void>;

  /** Start the single global 30fps polling loop for visualizer render data. */
  startRenderPolling: () => void;
  /** Stop the polling loop and clear cached render data. */
  stopRenderPolling: () => void;
}

export const useAppStore = createWithEqualityFn<AppState>((set, get) => {
  const ctxGetDef: ValidationContext["getDef"] = (type) =>
    (type ? (nodeDefs as any)[type] : undefined);

  /**
   * Push the graph to the Rust runtime only when every validated node passes.
   * Nodes with no validation entry are assumed valid (not yet processed).
   * Called after every cascade/full-validation pass.
   */
  function pushToRuntimeIfValid(
    nodes: NodeType[],
    edges: EdgeType[],
    validation: Record<string, ValidationResult>,
  ): void {
    const allOk = Object.values(validation).every((v) => v.ok);
    if (!allOk) return;
    fireAndForget(
      invoke("replace_graph", {
        nodes: nodes.map(serializeNode),
        edges: edges.map(serializeEdge),
      }),
      "replace_graph (valid graph)",
    );
  }

  /**
   * Run the validation cascade from `seedIds` against the current store state
   * and apply the resulting nodes/edges/validation atomically.
   * If the resulting graph is fully valid, push it to the Rust runtime.
   */
  function applyCascade(seedIds: string[]): void {
    const { nodes, edges, validation } = get();
    const out = runCascade({ nodes, edges, validation, getDef: ctxGetDef }, seedIds);
    set({
      nodes: out.nodes as NodeType[],
      edges: out.edges as EdgeType[],
      validation: out.validation,
    });
    pushToRuntimeIfValid(out.nodes as NodeType[], out.edges as EdgeType[], out.validation);
  }

  function applyFullValidation(): void {
    const { nodes, edges } = get();
    const out = runFullValidation({ nodes, edges, validation: {}, getDef: ctxGetDef });
    set({
      nodes: out.nodes as NodeType[],
      edges: out.edges as EdgeType[],
      validation: out.validation,
    });
    pushToRuntimeIfValid(out.nodes as NodeType[], out.edges as EdgeType[], out.validation);
  }

  return {
  menuOpen: false,

  contextMenuOpen: false,
  contextMenuPosition: { x: 0, y: 0 },
  contextMenuFlowPosition: { x: 0, y: 0 },
  contextMenuTargetNodeId: null,

  availableAudioHosts: null,
  selectedAudioHost: null,

  bufferSize: DEFAULT_BUFFER_SIZE,

  availableAudioInputDevices: null,
  availableAudioOutputDevices: null,

  driverConnected: false,
  virtualDevices: [],
  vstPluginList: [],
  nodeRenderData: {},
  bluetoothBattery: {},
  bluetoothBatteryEnabled: false,
  minimizeToTrayEnabled: false,
  validation: {},

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
    const isGain = type === "gain";
    const isChannelMerge = type === "channelMerge";
    const isChannelSplit = type === "channelSplit";
    const isDelay = type === "delay";
    const isCompressor = type === "compressor";
    const isReverb = type === "reverb";
    const isEcho = type === "echo";
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
              : isGain
                ? { gain: 1.0, edgeType: null }
                : isChannelMerge
                  ? { inputCount: 2 as const }
                  : isChannelSplit
                    ? { outputCount: 2 as const }
                    : isDelay
                    ? { delayMs: 250, edgeType: null }
                    : isCompressor
                      ? { thresholdDb: -12, ratio: 4, attackMs: 5, releaseMs: 50, makeUpDb: 0, edgeType: null }
                      : isReverb
                        ? { roomSize: 0.5, wet: 0.33, damp: 0.5, edgeType: null }
                        : isEcho
                          ? { delayMs: 375, feedback: 0.4, wet: 0.5, edgeType: null }
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
    applyCascade([newNode.id]);
  },

  removeNodeAtContextMenu: () => {
    const { nodes, edges, contextMenuTargetNodeId, validation } = get();

    if (!contextMenuTargetNodeId) {
      return;
    }

    // Capture neighbors before deleting so we can re-validate them after.
    const neighbors = new Set<string>();
    for (const e of edges) {
      if (e.source === contextMenuTargetNodeId) neighbors.add(e.target);
      if (e.target === contextMenuTargetNodeId) neighbors.add(e.source);
    }

    const { [contextMenuTargetNodeId]: _removed, ...remainingValidation } = validation;
    set({
      nodes: nodes.filter((node) => node.id !== contextMenuTargetNodeId),
      edges: edges.filter(
        (edge) =>
          edge.source !== contextMenuTargetNodeId && edge.target !== contextMenuTargetNodeId,
      ),
      validation: remainingValidation,
    });
    applyCascade([...neighbors]);
  },

  setSelectedAudioHost: (host: string) => {
    set({ selectedAudioHost: host });
    fireAndForget(persistSetting(SETTING_KEYS.audioHost, host), "persist audio host");
  },
  setBufferSize: (size: number) => {
    set({ bufferSize: size });
    fireAndForget(persistSetting(SETTING_KEYS.bufferSize, size), "persist buffer size");
  },
  setDriverConnected: (connected: boolean) => set({ driverConnected: connected }),

  setBluetoothBatteryEnabled: async (enabled: boolean) => {
    set({ bluetoothBatteryEnabled: enabled });
    try {
      await persistSetting(SETTING_KEYS.bluetoothBatteryEnabled, enabled);
    } catch (e) {
      console.warn("Failed to persist bluetoothBatteryEnabled:", e);
    }
    try {
      if (enabled) {
        await invoke("start_bluetooth_battery_watcher");
      } else {
        await invoke("stop_bluetooth_battery_watcher");
        set({ bluetoothBattery: {} });
      }
    } catch (e) {
      console.warn("Failed to toggle bluetooth battery watcher:", e);
    }
  },

  setBluetoothBattery: (info: BluetoothBatteryInfo) =>
    set((s) => ({ bluetoothBattery: { ...s.bluetoothBattery, [info.containerId]: info } })),

  setMinimizeToTrayEnabled: async (enabled: boolean) => {
    set({ minimizeToTrayEnabled: enabled });
    try {
      await persistSetting(SETTING_KEYS.minimizeToTray, enabled);
    } catch (e) {
      console.warn("Failed to persist minimizeToTrayEnabled:", e);
    }
  },

  initializeApp: async () => {
    // Hydrate persisted preferences first so subsequent initialization picks
    // them up. Failures fall through to defaults.
    const [persistedHost, persistedBuffer, persistedBtBattery, persistedTray] =
      await Promise.all([
        readSetting<string>(SETTING_KEYS.audioHost),
        readSetting<number>(SETTING_KEYS.bufferSize),
        readSetting<boolean>(SETTING_KEYS.bluetoothBatteryEnabled),
        readSetting<boolean>(SETTING_KEYS.minimizeToTray),
      ]);

    let savedHost: string | null = null;
    if (typeof persistedHost === "string") savedHost = persistedHost;
    if (typeof persistedBuffer === "number" &&
        (BUFFER_SIZE_OPTIONS as readonly number[]).includes(persistedBuffer)) {
      set({ bufferSize: persistedBuffer });
    }
    const btBatteryEnabled = typeof persistedBtBattery === "boolean" && persistedBtBattery;
    if (typeof persistedBtBattery === "boolean") {
      set({ bluetoothBatteryEnabled: persistedBtBattery });
    }
    if (typeof persistedTray === "boolean") {
      set({ minimizeToTrayEnabled: persistedTray });
    }

    if (btBatteryEnabled) {
      invoke("start_bluetooth_battery_watcher").catch((e) =>
        console.warn("Failed to start bluetooth battery watcher:", e),
      );
    }

    async function initHosts() {
      const hosts = await invoke<string[]>("get_audio_hosts");
      const initial =
        savedHost && hosts.includes(savedHost) ? savedHost : hosts[0] || null;
      set({ availableAudioHosts: hosts, selectedAudioHost: initial });
      return initial;
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
      // Always load persisted virtual devices so they are available in the UI
      // even when the driver is offline.
      const persistedDevices = await readSetting<VirtualDevice[]>(SETTING_KEYS.virtualDevices);
      if (Array.isArray(persistedDevices) && persistedDevices.length > 0) {
        set({ virtualDevices: persistedDevices });
      }

      try {
        const connected = await invoke<boolean>("connect_driver");
        set({ driverConnected: connected });

        if (connected && Array.isArray(persistedDevices) && persistedDevices.length > 0) {
          // Repopulate the backend's in-memory device map from the persisted
          // list so that remove/rename/ring-buffer commands work correctly.
          await invoke("restore_virtual_devices", { devices: persistedDevices });
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

    // applyFullValidation will call replace_graph if the initial graph is valid.
    applyFullValidation();
  },

  addVirtualDevice: async (name, deviceType) => {
    try {
      const device = await invoke<VirtualDevice>("create_virtual_device", {
        name,
        deviceType,
      });
      const updated = [...get().virtualDevices, device];
      set({ virtualDevices: updated });
      fireAndForget(
        persistSetting(SETTING_KEYS.virtualDevices, updated),
        "persist virtual devices (add)",
      );
    } catch (e) {
      console.error("Failed to create virtual device:", e);
      throw e;
    }
  },

  removeVirtualDevice: async (deviceId) => {
    try {
      await invoke("remove_virtual_device", { deviceId });
      const updated = get().virtualDevices.filter((d) => d.id !== deviceId);
      set({ virtualDevices: updated });
      fireAndForget(
        persistSetting(SETTING_KEYS.virtualDevices, updated),
        "persist virtual devices (remove)",
      );
    } catch (e) {
      console.error("Failed to remove virtual device:", e);
      throw e;
    }
  },

  renameVirtualDevice: async (deviceId, newName) => {
    try {
      await invoke("rename_virtual_device", { deviceId, newName });
      const updated = get().virtualDevices.map((d) =>
        d.id === deviceId ? { ...d, name: newName } : d,
      );
      set({ virtualDevices: updated });
      fireAndForget(
        persistSetting(SETTING_KEYS.virtualDevices, updated),
        "persist virtual devices (rename)",
      );
    } catch (e) {
      console.error("Failed to rename virtual device:", e);
      throw e;
    }
  },

  setVirtualDeviceFormat: async (deviceId, channels, sampleRate, bitsPerSample) => {
    try {
      await invoke("set_virtual_device_format", {
        deviceId,
        channels,
        sampleRate,
        bitsPerSample,
      });
    } catch (e) {
      // Backend command may fail when the driver is not connected; proceed
      // with updating local state regardless so the UI stays consistent.
      console.warn("set_virtual_device_format backend call failed:", e);
    }

    // Update the device's format in the store.
    const updated = get().virtualDevices.map((d) =>
      d.id === deviceId ? { ...d, channels, sampleRate, bitsPerSample } : d,
    );
    set({ virtualDevices: updated });

    // Propagate the new format to any graph nodes that reference this device
    // so the validation engine picks up the change immediately.
    const nodeIds: string[] = [];
    const newNodes = get().nodes.map((n) => {
      const data = n.data as any;
      if (data?.deviceId === deviceId) {
        nodeIds.push(n.id);
        return { ...n, data: { ...data, channels, sampleRate, bitsPerSample } };
      }
      return n;
    });
    set({ nodes: newNodes as any });

    if (nodeIds.length > 0) {
      applyCascade(nodeIds);
    }

    fireAndForget(
      persistSetting(SETTING_KEYS.virtualDevices, updated),
      "persist virtual devices (format)",
    );
  },

  onNodesChange: (changes) => {
    const prev = get().nodes;
    const next = applyNodeChanges<NodeType>(changes, prev);

    // Remove stale validation entries for deleted nodes so they don't block
    // the "all ok" check in pushToRuntimeIfValid.
    const currentValidation = get().validation;
    const newValidation = { ...currentValidation };
    const seeds: string[] = [];
    let hasRemovals = false;
    for (const change of changes) {
      if (change.type === "remove") {
        delete newValidation[change.id];
        hasRemovals = true;
      }
      // `replace` carries a new full node object; `select`/`position`/`dimensions`
      // never affect data, so we don't bother seeding them.
      if (change.type === "replace") seeds.push(change.id);
    }
    set({ nodes: next, validation: newValidation });
    if (seeds.length) {
      applyCascade(seeds);
    } else if (hasRemovals) {
      // No cascade seeds but nodes were removed — check validity with cleaned state.
      pushToRuntimeIfValid(next, get().edges, newValidation);
    }
  },

  onEdgesChange: (changes) => {
    const prev = get().edges;
    const next = applyEdgeChanges<EdgeType>(changes, prev);
    set({ edges: next });

    const removedSeeds = new Set<string>();
    for (const change of changes) {
      if (change.type === "remove") {
        const removedEdge = prev.find((e) => e.id === change.id);
        if (removedEdge) {
          removedSeeds.add(removedEdge.source);
          removedSeeds.add(removedEdge.target);
        }
      }
    }
    if (removedSeeds.size) applyCascade([...removedSeeds]);
  },

  onConnect: (connection) => {
    const { edges } = get();

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

    const nextEdges = addEdge({ ...connection, type: "audio" }, edges);
    set({ edges: nextEdges });

    // Cascade from both source and target — source's produced may now be
    // observable downstream, and target's expected may have shifted.
    applyCascade(
      [connection.source, connection.target].filter((s): s is string => Boolean(s)),
    );
  },

  updateNode: (id: string, data: any) => {
    const nextNodes = get().nodes.map((node) =>
      node.id === id ? { ...node, data: { ...node.data, ...data } } : node,
    );
    set({ nodes: nextNodes });
    applyCascade([id]);
  },

  loadGraph: (nodes: NodeType[], edges: EdgeType[]) => {
    const typedEdges = edges.map((e) => (e.type ? e : { ...e, type: "audio" }));
    set({ nodes, edges: typedEdges, validation: {} });
    fireAndForget(
      invoke("replace_graph", {
        nodes: nodes.map(serializeNode),
        edges: typedEdges.map(serializeEdge),
      }),
      "replace_graph",
    );
    applyFullValidation();
  },

  runFullTypeCheck: () => {
    applyFullValidation();
  },

  scanVstPlugins: async () => {
    const plugins = (await invoke("plugin_command", {
      pluginType: "vst",
      data: { op: "scan" },
    })) as VstPluginInfo[];
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
};
});
