import { invoke } from "@tauri-apps/api/core";
import {
  CheckIcon,
  ChevronDownIcon,
  ChevronUpIcon,
  PencilIcon,
  PlusIcon,
  Settings as SettingsIcon,
  TrashIcon,
  XIcon,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";

import { cn } from "@/lib/utils";
import { BUFFER_SIZE_OPTIONS, useAppStore } from "@/state";
import { VirtualDevice } from "@/types";

type CategoryId = "general" | "audio" | "virtualDevices" | "developer";

interface Category {
  id: CategoryId;
  label: string;
}

const CATEGORIES: Category[] = [
  { id: "general", label: "General" },
  { id: "audio", label: "Audio" },
  { id: "virtualDevices", label: "Virtual Devices" },
  { id: "developer", label: "Developer" },
];

function DeviceItem({
  device,
  onRemove,
  onRename,
  onSetFormat,
}: {
  device: VirtualDevice;
  onRemove: () => void;
  onRename: (newName: string) => Promise<void>;
  onSetFormat: (channels: number, sampleRate: number, bitsPerSample: number) => Promise<void>;
}) {
  const [editing, setEditing] = useState(false);
  const [editName, setEditName] = useState(device.name);
  const [renaming, setRenaming] = useState(false);
  const [renameError, setRenameError] = useState<string | null>(null);
  const [formatOpen, setFormatOpen] = useState(false);

  const channels = device.channels ?? 2;
  const sampleRate = device.sampleRate ?? 48000;
  const bitsPerSample = device.bitsPerSample ?? 32;

  const handleConfirm = async () => {
    if (!editName.trim() || editName === device.name) {
      setEditing(false);
      return;
    }
    setRenaming(true);
    setRenameError(null);
    try {
      await onRename(editName.trim());
      setEditing(false);
    } catch (e: unknown) {
      const msg = String(e);
      if (msg.includes("cancelled UAC") || msg.includes("ShellExecuteExW")) {
        setRenameError("Rename cancelled (UAC denied)");
      } else {
        setRenameError("Rename failed");
      }
    } finally {
      setRenaming(false);
    }
  };

  return (
    <div className="flex flex-col gap-0.5">
      <div className="flex items-center gap-1 group">
        {editing ? (
          <>
            <input
              type="text"
              className="flex-1 min-w-0 px-1 py-0.5 text-xs border border-gray-300 rounded bg-white text-black"
              value={editName}
              onChange={(e) => setEditName(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") handleConfirm();
                if (e.key === "Escape") {
                  setEditing(false);
                  setRenameError(null);
                }
              }}
              disabled={renaming}
              autoFocus
            />
            <button
              className="p-0.5 text-green-600 hover:text-green-800 disabled:opacity-40"
              onClick={handleConfirm}
              disabled={renaming}
              title={renaming ? "Renaming…" : "Confirm"}
            >
              <CheckIcon size={12} />
            </button>
          </>
        ) : (
          <>
            <span className="flex-1 min-w-0 text-xs text-black truncate" title={device.name}>
              {device.name}
            </span>
            <button
              className="p-0.5 text-gray-400 hover:text-gray-700 opacity-0 group-hover:opacity-100 transition-opacity"
              onClick={() => setFormatOpen((v) => !v)}
              title="Format preset"
            >
              {formatOpen ? <ChevronUpIcon size={12} /> : <ChevronDownIcon size={12} />}
            </button>
            <button
              className="p-0.5 text-gray-400 hover:text-gray-700 opacity-0 group-hover:opacity-100 transition-opacity"
              onClick={() => {
                setEditName(device.name);
                setRenameError(null);
                setEditing(true);
              }}
              title="Rename"
            >
              <PencilIcon size={12} />
            </button>
            <button
              className="p-0.5 text-gray-400 hover:text-red-600 opacity-0 group-hover:opacity-100 transition-opacity"
              onClick={onRemove}
              title="Remove"
            >
              <TrashIcon size={12} />
            </button>
          </>
        )}
      </div>
      {renameError && <span className="text-xs text-red-500 pl-1">{renameError}</span>}
      {formatOpen && (
        <div className="pl-1 pt-0.5 pb-1 flex flex-col gap-1 bg-gray-50 rounded border border-gray-200">
          <span className="text-[10px] font-semibold text-gray-500 uppercase tracking-wide px-1">
            Format Preset
          </span>
          <div className="flex items-center gap-1 px-1">
            <label className="text-[10px] text-gray-500 w-16 shrink-0">Channels</label>
            <select
              className="flex-1 text-[10px] border border-gray-300 rounded bg-white text-black px-0.5 py-0.5"
              value={channels}
              onChange={(e) => {
                void onSetFormat(Number(e.target.value), sampleRate, bitsPerSample);
              }}
            >
              <option value={1}>1 (Mono)</option>
              <option value={2}>2 (Stereo)</option>
            </select>
          </div>
          <div className="flex items-center gap-1 px-1">
            <label className="text-[10px] text-gray-500 w-16 shrink-0">Sample Rate</label>
            <select
              className="flex-1 text-[10px] border border-gray-300 rounded bg-white text-black px-0.5 py-0.5"
              value={sampleRate}
              onChange={(e) => {
                void onSetFormat(channels, Number(e.target.value), bitsPerSample);
              }}
            >
              <option value={44100}>44100 Hz</option>
              <option value={48000}>48000 Hz</option>
              <option value={88200}>88200 Hz</option>
              <option value={96000}>96000 Hz</option>
              <option value={192000}>192000 Hz</option>
            </select>
          </div>
          <div className="flex items-center gap-1 px-1">
            <label className="text-[10px] text-gray-500 w-16 shrink-0">Bit Depth</label>
            <select
              className="flex-1 text-[10px] border border-gray-300 rounded bg-white text-black px-0.5 py-0.5"
              value={bitsPerSample}
              onChange={(e) => {
                void onSetFormat(channels, sampleRate, Number(e.target.value));
              }}
            >
              <option value={16}>16-bit</option>
              <option value={24}>24-bit</option>
              <option value={32}>32-bit</option>
            </select>
          </div>
        </div>
      )}
    </div>
  );
}

function DeviceGroup({
  title,
  color,
  devices,
  deviceType,
  onAdd,
  onRemove,
  onRename,
  onSetFormat,
}: {
  title: string;
  color: string;
  devices: VirtualDevice[];
  deviceType: "render" | "capture";
  onAdd: (name: string, type: "render" | "capture") => void;
  onRemove: (id: string) => void;
  onRename: (id: string, name: string) => Promise<void>;
  onSetFormat: (id: string, channels: number, sampleRate: number, bitsPerSample: number) => Promise<void>;
}) {
  const [adding, setAdding] = useState(false);
  const [newName, setNewName] = useState("");

  const handleAdd = () => {
    if (newName.trim()) {
      onAdd(newName.trim(), deviceType);
      setNewName("");
      setAdding(false);
    }
  };

  return (
    <div className="flex flex-col gap-1">
      <div className="flex items-center gap-1.5">
        <div className={cn("w-2 h-2 rounded-full", color)} />
        <span className="text-xs font-semibold text-gray-600 flex-1">{title}</span>
        <span className="text-xs text-gray-400">{devices.length}</span>
      </div>

      {devices.map((device) => (
        <div key={device.id} className="pl-3">
          <DeviceItem
            device={device}
            onRemove={() => onRemove(device.id)}
            onRename={(name) => onRename(device.id, name)}
            onSetFormat={(ch, sr, bps) => onSetFormat(device.id, ch, sr, bps)}
          />
        </div>
      ))}

      {adding ? (
        <div className="pl-3 flex items-center gap-1">
          <input
            type="text"
            className="flex-1 min-w-0 px-1 py-0.5 text-xs border border-gray-300 rounded bg-white text-black"
            placeholder="Device name..."
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") handleAdd();
              if (e.key === "Escape") {
                setAdding(false);
                setNewName("");
              }
            }}
            autoFocus
          />
          <button
            className="p-0.5 text-green-600 hover:text-green-800"
            onClick={handleAdd}
            title="Create"
          >
            <CheckIcon size={12} />
          </button>
        </div>
      ) : (
        <button
          className="pl-3 flex items-center gap-1 text-xs text-gray-400 hover:text-gray-700"
          onClick={() => setAdding(true)}
        >
          <PlusIcon size={12} />
          <span>Add {deviceType === "render" ? "speaker" : "mic"}</span>
        </button>
      )}
    </div>
  );
}

function GeneralPane() {
  const {
    bluetoothBatteryEnabled,
    setBluetoothBatteryEnabled,
    minimizeToTrayEnabled,
    setMinimizeToTrayEnabled,
  } = useAppStore();
  return (
    <div className="flex flex-col gap-3">
      <h2 className="text-sm font-bold text-black">General</h2>
      <label className="flex items-center gap-2 cursor-pointer text-xs text-black">
        <input
          type="checkbox"
          className="cursor-pointer"
          checked={minimizeToTrayEnabled}
          onChange={(e) => {
            void setMinimizeToTrayEnabled(e.target.checked);
          }}
        />
        <span className="flex-1">Keep running in tray when window is closed</span>
      </label>
      <label className="flex items-center gap-2 cursor-pointer text-xs text-black">
        <input
          type="checkbox"
          className="cursor-pointer"
          checked={bluetoothBatteryEnabled}
          onChange={(e) => {
            void setBluetoothBatteryEnabled(e.target.checked);
          }}
        />
        <span className="flex-1">
          Show AirPods battery on Bluetooth audio nodes
          <span className="block text-[10px] text-gray-500">
            Listens to Apple Continuity BLE advertisements while enabled.
          </span>
        </span>
      </label>
    </div>
  );
}

function AudioPane() {
  const {
    availableAudioHosts,
    selectedAudioHost,
    bufferSize,
    setSelectedAudioHost,
    setBufferSize,
  } = useAppStore();

  return (
    <div className="flex flex-col gap-4">
      <h2 className="text-sm font-bold text-black">Audio</h2>

      <div className="flex flex-col gap-1">
        <span className="text-xs font-semibold text-gray-600">Audio Host</span>
        <select
          className="border border-gray-300 rounded text-black text-xs p-1"
          onChange={(e) => setSelectedAudioHost(e.target.value)}
          value={selectedAudioHost ?? ""}
        >
          {availableAudioHosts ? (
            availableAudioHosts.map((host) => (
              <option key={host} value={host}>
                {host}
              </option>
            ))
          ) : (
            <option>Loading...</option>
          )}
        </select>
      </div>

      <div className="flex flex-col gap-1">
        <span className="text-xs font-semibold text-gray-600">Buffer Size</span>
        <select
          className="border border-gray-300 rounded text-black text-xs p-1"
          value={bufferSize}
          onChange={(e) => setBufferSize(Number(e.target.value))}
        >
          {BUFFER_SIZE_OPTIONS.map((size) => (
            <option key={size} value={size}>
              {size} frames
            </option>
          ))}
        </select>
        <span className="text-xs text-gray-400">
          Smaller buffers reduce latency but may cause audio dropouts.
        </span>
      </div>
    </div>
  );
}

function VirtualDevicesPane() {
  const {
    driverConnected,
    virtualDevices,
    addVirtualDevice,
    removeVirtualDevice,
    renameVirtualDevice,
    setVirtualDeviceFormat,
  } = useAppStore();

  const renderDevices = virtualDevices.filter((d) => d.deviceType === "render");
  const captureDevices = virtualDevices.filter((d) => d.deviceType === "capture");

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center justify-between">
        <h2 className="text-sm font-bold text-black">Virtual Devices</h2>
        <span
          className={cn(
            "text-xs px-1.5 py-0.5 rounded",
            driverConnected ? "bg-green-100 text-green-700" : "bg-red-100 text-red-700",
          )}
        >
          {driverConnected ? "connected" : "offline"}
        </span>
      </div>

      {!driverConnected && (
        <p className="text-xs text-gray-400">
          Driver not connected. Changes will not reach the driver.
        </p>
      )}

      <DeviceGroup
        title="Output (Speakers)"
        color="bg-teal-400"
        devices={renderDevices}
        deviceType="render"
        onAdd={addVirtualDevice}
        onRemove={removeVirtualDevice}
        onRename={renameVirtualDevice}
        onSetFormat={setVirtualDeviceFormat}
      />

      <DeviceGroup
        title="Input (Microphones)"
        color="bg-purple-400"
        devices={captureDevices}
        deviceType="capture"
        onAdd={addVirtualDevice}
        onRemove={removeVirtualDevice}
        onRename={renameVirtualDevice}
        onSetFormat={setVirtualDeviceFormat}
      />
    </div>
  );
}

function DeveloperPane() {
  return (
    <div className="flex flex-col gap-3">
      <h2 className="text-sm font-bold text-black">Developer</h2>
      <button
        className="self-start text-xs text-black border border-gray-300 rounded px-2 py-1 hover:bg-gray-100 transition-colors"
        onClick={() => invoke("open_devtools")}
      >
        Open Developer Tools
      </button>
    </div>
  );
}

export default function Menu() {
  const { menuOpen, setMenuOpen } = useAppStore();
  const [activeCategory, setActiveCategory] = useState<CategoryId>("general");

  // Close on Escape.
  useEffect(() => {
    if (!menuOpen) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setMenuOpen(false);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [menuOpen, setMenuOpen]);

  const pane = useMemo(() => {
    switch (activeCategory) {
      case "general":
        return <GeneralPane />;
      case "audio":
        return <AudioPane />;
      case "virtualDevices":
        return <VirtualDevicesPane />;
      case "developer":
        return <DeveloperPane />;
    }
  }, [activeCategory]);

  return (
    <>
      <SettingsIcon
        className={cn(
          menuOpen && "hidden",
          "text-black absolute bottom-0 left-0 m-2 cursor-pointer",
        )}
        onClick={() => setMenuOpen(true)}
      />

      {menuOpen && (
        <div
          className="absolute inset-0 z-50 flex items-center justify-center bg-black/40"
          onClick={() => setMenuOpen(false)}
        >
          <div
            className="bg-white rounded-lg shadow-2xl border border-gray-200 flex overflow-hidden w-[640px] h-[420px] max-w-[90vw] max-h-[80vh]"
            onClick={(e) => e.stopPropagation()}
          >
            {/* Left: category list */}
            <div className="w-44 bg-gray-50 border-r border-gray-200 flex flex-col">
              <div className="px-3 py-2 border-b border-gray-200">
                <span className="text-sm font-bold text-black">Settings</span>
              </div>
              <nav className="flex-1 overflow-y-auto py-1">
                {CATEGORIES.map((cat) => (
                  <button
                    key={cat.id}
                    className={cn(
                      "w-full text-left px-3 py-1.5 text-xs transition-colors",
                      activeCategory === cat.id
                        ? "bg-blue-100 text-blue-900 font-semibold"
                        : "text-gray-700 hover:bg-gray-100",
                    )}
                    onClick={() => setActiveCategory(cat.id)}
                  >
                    {cat.label}
                  </button>
                ))}
              </nav>
            </div>

            {/* Right: settings pane */}
            <div className="flex-1 flex flex-col min-w-0">
              <div className="flex items-center justify-end px-2 py-1 border-b border-gray-200">
                <button
                  className="p-1 text-gray-500 hover:text-black cursor-pointer"
                  onClick={() => setMenuOpen(false)}
                  title="Close"
                >
                  <XIcon size={16} />
                </button>
              </div>
              <div className="flex-1 overflow-y-auto p-4">{pane}</div>
            </div>
          </div>
        </div>
      )}
    </>
  );
}
