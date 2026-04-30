// Mock implementation of `@tauri-apps/api/core` for Storybook.
// Vite aliases the real package to this file via .storybook/main.ts.

type Handler = (args: Record<string, unknown>) => unknown | Promise<unknown>;

const handlers: Record<string, Handler> = {
  get_window_list: async () => [
    { processId: 1234, title: "Notepad" },
    { processId: 5678, title: "Chrome — Storybook" },
    { processId: 9012, title: "Spotify" },
  ],
  get_audio_devices: async () => ({ inputs: [], outputs: [] }),
  is_driver_connected: async () => true,
  list_virtual_devices: async () => [],
  scan_vst_plugins: async () => [],
  node_command: async () => null,
  apply_graph: async () => null,
  set_runtime_enabled: async () => null,
  get_node_render_data: async () => ({}),
  rename_endpoint: async () => null,
  create_virtual_device: async () => null,
  remove_virtual_device: async () => null,
};

/** Override or add a mock handler for a Tauri command. Used inside stories. */
export function setInvokeHandler(command: string, handler: Handler): void {
  handlers[command] = handler;
}

/** Reset all handlers to defaults — call between stories if needed. */
export function resetInvokeHandlers(): void {
  // Intentionally a no-op for now; stories that override should restore.
}

export async function invoke<T = unknown>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  const handler = handlers[command];
  if (!handler) {
    console.warn(`[storybook tauri-mock] unhandled invoke: ${command}`, args);
    return undefined as T;
  }
  return (await handler(args ?? {})) as T;
}

// Re-export types/symbols that production code may import from this module.
export const Channel = class {};
export const PluginListener = class {};
export const Resource = class {};
