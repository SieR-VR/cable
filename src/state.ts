import { invoke } from "@tauri-apps/api/core";
import { Edge, Node, XYPosition } from "@xyflow/react";
import { proxy, useSnapshot } from "valtio";
import { AudioDevice } from "./types";

interface AppState {
  menuOpen: boolean;

  contextMenuOpen: boolean;
  contextMenuPosition: XYPosition;

  availableAudioHosts: string[] | null;
  selectedAudioHost: string | null;

  availableAudioInputDevices?: AudioDevice[] | null;
  availableAudioOutputDevices?: AudioDevice[] | null;
}

export const appState = proxy<AppState>({
  menuOpen: false,

  contextMenuOpen: false,
  contextMenuPosition: { x: 0, y: 0 },

  availableAudioHosts: null,
  selectedAudioHost: null,

  availableAudioInputDevices: null,
  availableAudioOutputDevices: null,
});

export const useAppState = () => useSnapshot(appState);

export function setMenuOpen(open: boolean) {
  appState.menuOpen = open;
}

export function setSelectedHost(host: string) {
  appState.selectedAudioHost = host;
}

export function initializeApp() {
  async function initHosts() {
    const hosts = await invoke("get_audio_hosts");
    appState.availableAudioHosts = hosts;
    appState.selectedAudioHost = hosts[0] || null;

    return hosts[0] || null;
  }

  async function initDevices(host: string | null) {
    if (!host) {
      appState.availableAudioInputDevices = null;
      return;
    }

    const [inputDevices, outputDevices] = await invoke("get_audio_devices", {
      host,
    });
    appState.availableAudioInputDevices = inputDevices;
    appState.availableAudioOutputDevices = outputDevices;
  }

  initHosts().then(initDevices);
}

export function setContextMenuOpen(
  open: boolean,
  position: XYPosition = { x: 0, y: 0 },
) {
  appState.contextMenuOpen = open;
  appState.contextMenuPosition = position;
}
