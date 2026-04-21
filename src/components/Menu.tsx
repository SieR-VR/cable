import { invoke } from "@tauri-apps/api/core";
import { MenuIcon, XIcon, PlusIcon, TrashIcon, PencilIcon, CheckIcon } from "lucide-react";
import { useState } from "react";

import { cn } from "@/lib/utils";
import { useAppStore } from "@/state";
import { VirtualDevice } from "@/types";

function DeviceItem({
  device,
  onRemove,
  onRename,
}: {
  device: VirtualDevice;
  onRemove: () => void;
  onRename: (newName: string) => Promise<void>;
}) {
  const [editing, setEditing] = useState(false);
  const [editName, setEditName] = useState(device.name);
  const [renaming, setRenaming] = useState(false);
  const [renameError, setRenameError] = useState<string | null>(null);

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
      // ShellExecuteExW failure means the user cancelled the UAC prompt.
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
}: {
  title: string;
  color: string;
  devices: VirtualDevice[];
  deviceType: "render" | "capture";
  onAdd: (name: string, type: "render" | "capture") => void;
  onRemove: (id: string) => void;
  onRename: (id: string, name: string) => Promise<void>;
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

export default function Menu() {
  const {
    menuOpen,
    availableAudioHosts,
    selectedAudioHost,
    driverConnected,
    virtualDevices,
    setMenuOpen,
    setSelectedAudioHost,
    addVirtualDevice,
    removeVirtualDevice,
    renameVirtualDevice,
  } = useAppStore();

  const renderDevices = virtualDevices.filter((d) => d.deviceType === "render");
  const captureDevices = virtualDevices.filter((d) => d.deviceType === "capture");

  return (
    <>
      <MenuIcon
        className={cn(menuOpen && "hidden", "text-black absolute top-0 m-2 cursor-pointer")}
        onClick={() => setMenuOpen(true)}
      />
      <div
        className={cn(
          menuOpen ? "transform-none" : "-translate-x-72",
          "absolute top-0 h-full w-72 bg-white border-r border-gray-200 shadow-lg transition-transform flex flex-col",
        )}
      >
        {/* Header */}
        <div className="flex items-center justify-between p-3 border-b border-gray-200">
          <span className="text-sm font-bold text-black">Cable</span>
          <XIcon
            className="text-gray-500 cursor-pointer hover:text-black"
            size={18}
            onClick={() => setMenuOpen(false)}
          />
        </div>

        <div className="flex-1 overflow-y-auto p-3 flex flex-col gap-4">
          {/* Audio Host */}
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

          {/* Separator */}
          <div className="h-px bg-gray-200" />

          {/* Virtual Devices */}
          <div className="flex flex-col gap-3">
            <div className="flex items-center justify-between">
              <span className="text-sm font-bold text-black">Virtual Devices</span>
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
            />

            <DeviceGroup
              title="Input (Microphones)"
              color="bg-purple-400"
              devices={captureDevices}
              deviceType="capture"
              onAdd={addVirtualDevice}
              onRemove={removeVirtualDevice}
              onRename={renameVirtualDevice}
            />
          </div>
        </div>

        {/* Footer: Dev Tools */}
        <div className="border-t border-gray-200 p-2">
          <button
            className="w-full text-xs text-gray-400 hover:text-gray-700 py-1 rounded hover:bg-gray-50 transition-colors"
            onClick={() => invoke("open_devtools")}
          >
            Developer Tools
          </button>
        </div>
      </div>
    </>
  );
}
