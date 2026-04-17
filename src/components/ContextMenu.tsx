import { useAppStore } from "@/state";

export function ContextMenu() {
  const {
    contextMenuOpen,
    contextMenuPosition,
    contextMenuTargetNodeId,
    addNodeAtContextMenu,
    removeNodeAtContextMenu,
    setContextMenuOpen,
    driverConnected,
  } = useAppStore();

  if (!contextMenuOpen) {
    return null;
  }

  return (
    <div
      className="fixed min-w-56 bg-white border border-gray-300 shadow-lg rounded-md p-2"
      style={{ top: contextMenuPosition.y, left: contextMenuPosition.x }}
    >
      <div className="px-2 pb-1 text-xs font-semibold text-gray-500">Add Node</div>
      <button
        className="w-full text-left px-4 py-2 hover:bg-gray-100 cursor-pointer rounded"
        onClick={() => {
          addNodeAtContextMenu("audioInputDevice");
          setContextMenuOpen(false);
        }}
      >
        Audio Input Device
      </button>
      <button
        className="w-full text-left px-4 py-2 hover:bg-gray-100 cursor-pointer rounded"
        onClick={() => {
          addNodeAtContextMenu("audioOutputDevice");
          setContextMenuOpen(false);
        }}
      >
        Audio Output Device
      </button>
      <div className="my-2 h-px bg-gray-200" />
      <div className="px-2 pb-1 text-xs font-semibold text-gray-500">
        Virtual Devices {!driverConnected && <span className="text-yellow-500">(driver offline)</span>}
      </div>
      <button
        className="w-full text-left px-4 py-2 hover:bg-gray-100 cursor-pointer rounded disabled:opacity-40 disabled:cursor-not-allowed"
        disabled={!driverConnected}
        onClick={() => {
          addNodeAtContextMenu("virtualAudioInput");
          setContextMenuOpen(false);
        }}
      >
        Virtual Mic (Capture)
      </button>
      <button
        className="w-full text-left px-4 py-2 hover:bg-gray-100 cursor-pointer rounded disabled:opacity-40 disabled:cursor-not-allowed"
        disabled={!driverConnected}
        onClick={() => {
          addNodeAtContextMenu("virtualAudioOutput");
          setContextMenuOpen(false);
        }}
      >
        Virtual Speaker (Render)
      </button>
      <div className="my-2 h-px bg-gray-200" />
      <button
        className="w-full text-left px-4 py-2 hover:bg-gray-100 cursor-pointer rounded disabled:opacity-40 disabled:cursor-not-allowed"
        disabled={!contextMenuTargetNodeId}
        onClick={() => {
          removeNodeAtContextMenu();
          setContextMenuOpen(false);
        }}
      >
        Remove Node
      </button>
    </div>
  );
}
