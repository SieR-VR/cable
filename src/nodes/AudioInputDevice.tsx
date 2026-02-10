import { useAppState } from "@/state";
import { Handle, Position } from "@xyflow/react";

export default function AudioInputDevice() {
  const { availableAudioInputDevices } = useAppState();

  return (
    <div className="h-32 bg-gray-700 rounded-lg flex flex-col items-center text-white">
      {/* Header */}
      <div className="w-full h-6 bg-red-400 rounded-t-lg flex items-center text-sm font-bold p-2 drag-handle__custom">
        Audio input device
      </div>
      <div className="w-full flex flex-col p-2">
        <select className="w-full p-1 rounded bg-gray-500">
          {availableAudioInputDevices ? (
            availableAudioInputDevices.map((device) => (
              <option key={device.id} value={device.id}>
                {device.descriptions?.join("\n")}
              </option>
            ))
          ) : (
            <option disabled>Loading devices...</option>
          )}
        </select>
      </div>
      {!availableAudioInputDevices && (
        <div className="text-xs text-gray-400">{"Loading..."}</div>
      )}
      <Handle
        type="source"
        position={Position.Right}
        id="audio"
        className="w-4 h-4 bg-green-500 rounded-full"
      />
    </div>
  );
}
