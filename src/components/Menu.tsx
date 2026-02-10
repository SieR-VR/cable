import { MenuIcon, XIcon } from "lucide-react";

import { cn } from "@/lib/utils";
import { setMenuOpen, setSelectedHost, useAppState } from "@/state";

export default function Menu() {
  const { menuOpen, availableAudioHosts, selectedAudioHost } = useAppState();

  return (
    <>
      <MenuIcon
        className={cn(menuOpen && "hidden", "text-black absolute top-0 m-2")}
        onClick={() => setMenuOpen(true)}
      />
      <div
        className={cn(
          menuOpen ? "transform-none" : "-translate-x-64",
          "absolute top-0 h-full w-64 bg-white border border-black transition-transform flex flex-col gap-2 p-2",
        )}
      >
        <XIcon
          className="text-black self-end cursor-pointer"
          onClick={() => setMenuOpen(false)}
        />
        <span className="text-black font-bold text-sm">Select Audio Host</span>
        {/* dropdown menu */}
        <select
          className="border border-black rounded text-black"
          onChange={(e) => {
            console.log("selected host:", e.target.value);
            setSelectedHost(e.target.value);
          }}
          value={selectedAudioHost ?? ""}
        >
          {availableAudioHosts ? (
            availableAudioHosts.map((host) => (
              <option key={host} value={host}>
                {host}
              </option>
            ))
          ) : (
            <option key={null}>Loading...</option>
          )}
        </select>
      </div>
    </>
  );
}
