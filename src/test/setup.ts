import "@testing-library/jest-dom/vitest";
import { vi } from "vitest";

// Mock Tauri IPC layer so tests run without a Tauri runtime.
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));
