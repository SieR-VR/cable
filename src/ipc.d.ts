import { AudioDevice, AudioGraph, NodeRenderData, VirtualDevice, VstParamInfo, VstPluginInfo, WindowInfo } from "./types";

declare module "@tauri-apps/api/core" {
  declare function invoke(cmd: "get_window_list"): Promise<WindowInfo[]>;
  declare function invoke(cmd: "get_audio_hosts"): Promise<string[]>;

  declare function invoke(
    cmd: "get_audio_devices",
    args: {
      host: string;
    },
  ): Promise<[AudioDevice[], AudioDevice[]]>;

  declare function invoke(cmd: "connect_driver"): Promise<boolean>;

  declare function invoke(cmd: "is_driver_connected"): Promise<boolean>;

  declare function invoke(
    cmd: "setup_runtime",
    args: {
      graph: AudioGraph;
      host: string;
      buffer_size: number;
    },
  ): Promise<void>;

  declare function invoke(cmd: "disable_runtime"): Promise<void>;

  declare function invoke(cmd: "enable_runtime"): Promise<void>;

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

  declare function invoke(cmd: "scan_vst3_plugins"): Promise<VstPluginInfo[]>;

  declare function invoke(
    cmd: "create_node",
    args: { node: { type: string; data: Record<string, unknown> } },
  ): Promise<void>;

  declare function invoke(
    cmd: "open_vst_editor",
    args: { nodeId: string; pluginPath: string },
  ): Promise<void>;

  declare function invoke(
    cmd: "close_vst_editor",
    args: { nodeId: string },
  ): Promise<void>;

  declare function invoke(
    cmd: "get_vst_params",
    args: { nodeId: string },
  ): Promise<VstParamInfo[]>;

  declare function invoke(
    cmd: "set_vst_param",
    args: { nodeId: string; paramId: number; value: number },
  ): Promise<void>;
}
