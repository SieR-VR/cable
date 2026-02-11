import { useAppStore } from "@/state";

export function ContextMenu() {
  const { contextMenuOpen, contextMenuPosition } = useAppStore();
  if (!contextMenuOpen) {
    return null;
  }

  return (
    <div
      className="fixed bg-white border border-gray-300 shadow-lg rounded-md p-2"
      style={{ top: contextMenuPosition.y, left: contextMenuPosition.x }}
    >
      <div className="px-4 py-2 hover:bg-gray-100 cursor-pointer rounded">
        Add Node
      </div>
      <div className="px-4 py-2 hover:bg-gray-100 cursor-pointer rounded">
        Remove Node
      </div>
    </div>
  );
}
