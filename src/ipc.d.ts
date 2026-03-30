import { AudioDevice, AudioGraph } from "./types";

declare module "@tauri-apps/api/core" {
  declare function invoke(cmd: "get_audio_hosts"): Promise<string[]>;

  declare function invoke(
    cmd: "get_audio_devices",
    args: {
      host: string;
    },
  ): Promise<[AudioDevice[], AudioDevice[]]>;

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
}
