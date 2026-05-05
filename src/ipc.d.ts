import { AudioDevice, AudioEdge, AudioGraph, AudioNode, NodeRenderData, VirtualDevice, WindowInfo } from "./types";

declare module "@tauri-apps/api/core" {
  declare function invoke(cmd: "get_window_list"): Promise<WindowInfo[]>;
  declare function invoke(cmd: "get_audio_hosts"): Promise<string[]>;

  declare function invoke(
    cmd: "get_audio_devices",
    args: {
      host: string;
    },
  ): Promise<[AudioDevice[], AudioDevice[]]>;

  declare function invoke(
    cmd: "get_audio_device_bluetooth",
    args: { deviceId: string },
  ): Promise<import("./types").BluetoothInfo | null>;

  declare function invoke(cmd: "start_bluetooth_battery_watcher"): Promise<void>;
  declare function invoke(cmd: "stop_bluetooth_battery_watcher"): Promise<void>;

  declare function invoke(cmd: "connect_driver"): Promise<boolean>;

  declare function invoke(cmd: "is_driver_connected"): Promise<boolean>;

  declare function invoke(cmd: "disable_runtime"): Promise<void>;

  declare function invoke(cmd: "enable_runtime"): Promise<void>;

  declare function invoke(
    cmd: "add_node",
    args: { node: AudioNode },
  ): Promise<void>;

  declare function invoke(
    cmd: "remove_node",
    args: { nodeId: string },
  ): Promise<void>;

  declare function invoke(
    cmd: "update_node",
    args: { node: AudioNode },
  ): Promise<void>;

  declare function invoke(
    cmd: "add_edge",
    args: { edge: AudioEdge },
  ): Promise<void>;

  declare function invoke(
    cmd: "remove_edge",
    args: { edgeId: string },
  ): Promise<void>;

  declare function invoke(
    cmd: "replace_graph",
    args: { nodes: AudioNode[]; edges: AudioEdge[] },
  ): Promise<void>;

  declare function invoke(
    cmd: "set_audio_config",
    args: { host: string; bufferSize: number; sampleRate?: number },
  ): Promise<void>;

  declare function invoke(cmd: "list_virtual_devices"): Promise<VirtualDevice[]>;

  declare function invoke(
    cmd: "create_virtual_device",
    args: {
      name: string;
      deviceType: "render" | "capture";
    },
  ): Promise<VirtualDevice>;

  declare function invoke(
    cmd: "remove_virtual_device",
    args: {
      deviceId: string;
    },
  ): Promise<void>;

  declare function invoke(
    cmd: "rename_virtual_device",
    args: {
      deviceId: string;
      newName: string;
    },
  ): Promise<void>;

  declare function invoke(
    cmd: "restore_virtual_devices",
    args: {
      devices: VirtualDevice[];
    },
  ): Promise<void>;

  declare function invoke(
    cmd: "set_virtual_device_format",
    args: {
      deviceId: string;
      channels: number;
      sampleRate: number;
      bitsPerSample: number;
    },
  ): Promise<void>;

  /**
   * Read the current PKEY_AudioEngine_DeviceFormat from each virtual device's
   * Windows MM endpoint. Returns system-authoritative format values.
   * Each entry is [id, sampleRate, channels, bitsPerSample].
   */
  declare function invoke(
    cmd: "sync_virtual_device_formats",
  ): Promise<[string, number, number, number][]>;

  declare function invoke(cmd: "open_devtools"): Promise<void>;

  declare function invoke(cmd: "get_node_render_data"): Promise<Record<string, NodeRenderData>>;

  declare function invoke(
    cmd: "save_graph",
    args: { content: string },
  ): Promise<boolean>;

  declare function invoke(
    cmd: "read_text_file",
    args: { path: string },
  ): Promise<string>;

  declare function invoke(
    cmd: "plugin_command",
    args: { pluginType: string; data: Record<string, unknown> },
  ): Promise<unknown>;

  declare function invoke(
    cmd: "node_command",
    args: { nodeId: string; data: Record<string, unknown> },
  ): Promise<unknown>;
}
