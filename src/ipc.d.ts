import { AudioDevice, AudioGraph, NodeRenderData, VirtualDevice, WindowInfo } from "./types";

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
}
