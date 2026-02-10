import { AudioDevice } from "./types";

declare module "@tauri-apps/api/core" {
  declare function invoke(cmd: "get_audio_hosts"): Promise<string[]>;

  declare function invoke(
    cmd: "get_audio_devices",
    args: {
      host: string;
    },
  ): Promise<[AudioDevice[], AudioDevice[]]>;
}
