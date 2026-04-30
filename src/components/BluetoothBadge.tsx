import { invoke } from "@tauri-apps/api/core";
import { Zap } from "lucide-react";
import { useEffect, useState } from "react";

import { useAppStore } from "@/state";
import { AudioDevice, BluetoothBatteryInfo, BluetoothInfo } from "@/types";

/**
 * Resolve the Bluetooth identity (if any) of an audio device on the backend.
 * Result is cached per device id for the lifetime of the component.
 */
export function useBluetoothInfo(device: AudioDevice | null): BluetoothInfo | null {
  const [info, setInfo] = useState<BluetoothInfo | null>(null);
  useEffect(() => {
    setInfo(null);
    if (!device) return;
    let cancelled = false;
    invoke("get_audio_device_bluetooth", { deviceId: device.id })
      .then((bt) => {
        if (!cancelled) setInfo(bt ?? null);
      })
      .catch(() => {
        if (!cancelled) setInfo(null);
      });
    return () => {
      cancelled = true;
    };
  }, [device?.id]);
  return info;
}

interface BluetoothBadgeProps {
  info: BluetoothInfo | null;
}

const BLUETOOTH_GLYPH = "\u{1F50A}";

/**
 * Compact "this is a Bluetooth device" indicator shown under the audio device
 * picker. Renders nothing for non-BT devices.
 */
export function BluetoothBadge({ info }: BluetoothBadgeProps) {
  if (!info?.isBluetooth) return null;

  const segments: string[] = [];
  if (info.category) segments.push(info.category);
  if (info.vendorId === 0x004c) segments.push("Apple");
  else if (info.vendorId != null)
    segments.push(`VID 0x${info.vendorId.toString(16).padStart(4, "0").toUpperCase()}`);
  if (info.address) segments.push(info.address);

  return (
    <div
      className="mt-1 flex items-center gap-1 text-[10px] text-sky-300/90"
      title={info.containerId}
    >
      <span aria-label="Bluetooth" role="img">
        {BLUETOOTH_GLYPH}
      </span>
      <span className="truncate">{segments.join(" \u00b7 ") || "Bluetooth"}</span>
    </div>
  );
}

interface BatteryCellProps {
  label: string;
  value: number | null;
  charging: boolean;
}

function BatteryCell({ label, value, charging }: BatteryCellProps) {
  const known = value != null;
  return (
    <div className="flex flex-col items-center gap-0.5 min-w-[34px]">
      <span className="text-[9px] text-gray-400">{label}</span>
      <div className="relative w-7 h-3 border border-gray-400 rounded-sm bg-gray-800 overflow-hidden">
        {known && (
          <div
            className={
              "absolute left-0 top-0 bottom-0 " +
              (value! <= 20 ? "bg-rose-400" : value! <= 50 ? "bg-amber-300" : "bg-emerald-400")
            }
            style={{ width: `${Math.max(0, Math.min(100, value!))}%` }}
          />
        )}
        {charging && (
          <Zap className="absolute inset-0 m-auto text-yellow-200" size={9} strokeWidth={3} />
        )}
      </div>
      <span className="text-[9px] text-gray-300">{known ? `${value}%` : "—"}</span>
    </div>
  );
}

interface BluetoothBatteryWidgetProps {
  info: BluetoothInfo | null;
}

/**
 * Inline AirPods battery readout for audio device nodes. Renders nothing when
 * the AppleCP advertisement watcher has not yet cached values for this device.
 */
export function BluetoothBatteryWidget({ info }: BluetoothBatteryWidgetProps) {
  const containerId = info?.containerId ?? null;
  const battery = useAppStore((s) =>
    containerId ? (s.bluetoothBattery[containerId] ?? null) : null,
  ) as BluetoothBatteryInfo | null;
  if (!battery) return null;
  return (
    <div className="mt-1 flex items-center justify-around gap-1 px-1 py-1 rounded bg-gray-700/40">
      <BatteryCell label="L" value={battery.left} charging={battery.chargingLeft} />
      <BatteryCell label="R" value={battery.right} charging={battery.chargingRight} />
      <BatteryCell label="Case" value={battery.case} charging={battery.chargingCase} />
    </div>
  );
}
