import { Handle, NodeProps, Position } from "@xyflow/react";
import { AppState, useAppStore } from "@/state";

import { AudioDevice } from "@/types";
import { formatAudioEdgeId } from "@/lib/utils";

const selector = (id: string) => (store: AppState) => ({
  setDevice: (device: AudioDevice | null) => store.updateNode(id, { device }),
});

export default function AudioInputDevice({ id, data }: NodeProps) {
  const { availableAudioInputDevices } = useAppStore();

  const { setDevice } = useAppStore(selector(id));
  const selectedDevice = data.device as AudioDevice | null;

  const edgeId =
    selectedDevice &&
    formatAudioEdgeId(
      selectedDevice.frequency,
      selectedDevice.channels,
      selectedDevice.bits_per_sample,
    );

  return (
    <div className="bg-gray-700 rounded-lg flex flex-col text-white">
      {/* Header */}
      <div className="w-full h-6 bg-red-400 rounded-t-lg flex items-center text-sm font-bold p-2 drag-handle__custom">
        Audio input device
      </div>
      <div className="flex flex-col gap-2 p-2">
        <div className="w-full flex flex-col">
          <select
            className="w-full p-1 rounded bg-gray-500"
            onChange={(e) => {
              setDevice(
                availableAudioInputDevices?.find(
                  (device) => device.id === e.target.value,
                ) || null,
              );
            }}
          >
            {availableAudioInputDevices ? (
              <>
                <option value="">-- Select an audio input device --</option>
                {availableAudioInputDevices.map((device) => (
                  <option key={device.id} value={device.id}>
                    {device.descriptions?.join("\n")}
                  </option>
                ))}
              </>
            ) : (
              <option disabled>Loading devices...</option>
            )}
          </select>
        </div>
        {!availableAudioInputDevices && (
          <div className="text-xs text-gray-400">{"Loading..."}</div>
        )}
        {selectedDevice && (
          <div className="flex flex-row gap-2 items-center">
            <span className="rounded-md text-xs bg-amber-200 p-1">{`${selectedDevice.frequency}Hz`}</span>
            <span className="rounded-md text-xs bg-blue-200 p-1">{`${selectedDevice.channels}ch`}</span>
            <span className="rounded-md text-xs bg-lime-200 p-1">{`${selectedDevice.bits_per_sample}bit`}</span>
          </div>
        )}
        <Handle
          type="source"
          position={Position.Right}
          id={edgeId}
          className="w-4 h-4 bg-green-500 rounded-full"
        />
      </div>
    </div>
  );
}
