import type { Meta, StoryObj } from "@storybook/react-vite";
import type { CSSProperties, ReactNode } from "react";

/**
 * Visual-only design proposals for the node card chrome.
 *
 * Goal: with handles + edges already conveying channel / rate / bit info,
 * the node body no longer needs format pills. These variants explore
 * different ways to identify a node's *kind* (the colored accent) without
 * relying on tag chips.
 */

interface VariantProps {
  accent: string;
  accent2?: string;
  title: string;
  rightHandle?: boolean;
  leftHandle?: boolean;
  children?: ReactNode;
}

function FakeHandleStack({
  color,
  side,
  count = 2,
}: {
  color: string;
  side: "left" | "right";
  count?: number;
}) {
  const offsets = count === 1 ? [0] : count === 2 ? [-3, 3] : [-5, 0, 5];
  const PILL_PAD = 2.5;
  const along = (Math.max(...offsets.map(Math.abs)) + 2 + PILL_PAD) * 2;
  const wrap: CSSProperties = {
    position: "absolute",
    [side]: -9,
    top: "50%",
    transform: "translateY(-50%)",
    width: 18,
    height: 18,
    pointerEvents: "none",
  };
  return (
    <span style={wrap}>
      {count >= 2 && (
        <span
          style={{
            position: "absolute",
            left: "50%",
            top: "50%",
            transform: "translate(-50%, -50%)",
            width: 4 + PILL_PAD * 2,
            height: along,
            borderRadius: 9999,
            border: "1px solid #6e7681",
            opacity: 0.6,
          }}
        />
      )}
      {offsets.map((off, i) => (
        <span
          key={i}
          style={{
            position: "absolute",
            left: "50%",
            top: "50%",
            transform: `translate(-50%, calc(-50% + ${off}px))`,
            width: 4,
            height: 4,
            borderRadius: "50%",
            background: color,
          }}
        />
      ))}
    </span>
  );
}

function MockSelect({ placeholder }: { placeholder: string }) {
  return (
    <div className="bg-gray-600 rounded text-xs text-white px-2 py-1 flex items-center justify-between">
      <span className="truncate">{placeholder}</span>
      <span className="text-gray-300 ml-2">▾</span>
    </div>
  );
}

function MultiHandleRows({ accent, names }: { accent: string; names: string[] }) {
  return (
    <div className="flex flex-col gap-1">
      {names.map((name) => (
        <div key={name} className="flex items-center gap-2 h-5 relative">
          <FakeHandleStack color={accent} side="left" />
          <span className="text-xs text-gray-300 ml-2">{name}</span>
        </div>
      ))}
    </div>
  );
}

// ---------- variant chromes ---------------------------------------------

function VariantA(props: VariantProps) {
  return (
    <div className="relative bg-gray-700 rounded-lg flex flex-col text-white min-w-44 shadow-md">
      <div
        className="w-full h-6 rounded-t-lg flex items-center text-sm font-bold px-2"
        style={{ background: props.accent }}
      >
        {props.title}
      </div>
      <div className="flex flex-col gap-2 p-2 relative">{props.children}</div>
      {props.rightHandle && <FakeHandleStack color={props.accent} side="right" />}
      {props.leftHandle && <FakeHandleStack color={props.accent} side="left" />}
    </div>
  );
}

function VariantB(props: VariantProps) {
  return (
    <div
      className="relative bg-gray-700 rounded-lg flex flex-col text-white min-w-44 shadow-md overflow-hidden"
      style={{ borderLeft: `3px solid ${props.accent}` }}
    >
      <div className="flex flex-col gap-2 p-2 relative">
        <div className="text-xs font-semibold text-gray-200 tracking-wide uppercase">
          {props.title}
        </div>
        {props.children}
      </div>
      {props.rightHandle && <FakeHandleStack color={props.accent} side="right" />}
      {props.leftHandle && <FakeHandleStack color={props.accent} side="left" />}
    </div>
  );
}

function VariantC(props: VariantProps) {
  const initial = props.title.match(/[A-Z]/)?.[0] ?? props.title[0];
  return (
    <div className="relative bg-gray-800 rounded-lg flex flex-col text-white min-w-44 shadow-md border border-gray-700">
      <div className="flex items-center gap-2 px-2 py-1.5 border-b border-gray-700">
        <div
          className="w-5 h-5 rounded flex items-center justify-center text-[11px] font-bold text-white"
          style={{ background: props.accent }}
        >
          {initial}
        </div>
        <span className="text-xs font-semibold text-gray-100">{props.title}</span>
      </div>
      <div className="flex flex-col gap-2 p-2 relative">{props.children}</div>
      {props.rightHandle && <FakeHandleStack color={props.accent} side="right" />}
      {props.leftHandle && <FakeHandleStack color={props.accent} side="left" />}
    </div>
  );
}

function VariantD(props: VariantProps) {
  const [first, ...rest] = props.title.split(" ");
  return (
    <div className="relative bg-gray-700/95 rounded-lg flex flex-col text-white min-w-44 shadow-md">
      <div className="flex items-center gap-2 px-2 pt-2">
        <span
          className="text-[10px] font-bold uppercase px-1.5 py-0.5 rounded-full text-white tracking-wider"
          style={{ background: props.accent }}
        >
          {first}
        </span>
        {rest.length > 0 && (
          <span className="text-xs text-gray-300 truncate">{rest.join(" ")}</span>
        )}
      </div>
      <div className="flex flex-col gap-2 p-2 relative">{props.children}</div>
      {props.rightHandle && <FakeHandleStack color={props.accent} side="right" />}
      {props.leftHandle && <FakeHandleStack color={props.accent} side="left" />}
    </div>
  );
}

function VariantE(props: VariantProps) {
  const grad = props.accent2
    ? `linear-gradient(135deg, ${props.accent}33, ${props.accent2}33)`
    : `linear-gradient(135deg, ${props.accent}33, transparent)`;
  return (
    <div className="relative rounded-lg flex flex-col text-white min-w-44 shadow-md border border-gray-700 overflow-hidden bg-gray-800">
      <div
        className="flex items-center gap-2 px-2 py-1.5 border-b border-gray-700"
        style={{ background: grad }}
      >
        <span
          className="w-2 h-2 rounded-full"
          style={{ background: props.accent, boxShadow: `0 0 6px ${props.accent}` }}
        />
        <span className="text-xs font-semibold text-gray-100">{props.title}</span>
      </div>
      <div className="flex flex-col gap-2 p-2 relative">{props.children}</div>
      {props.rightHandle && <FakeHandleStack color={props.accent} side="right" />}
      {props.leftHandle && <FakeHandleStack color={props.accent} side="left" />}
    </div>
  );
}

// ---------- gallery ----------------------------------------------------

const VARIANTS: Array<{
  key: string;
  title: string;
  description: string;
  Variant: (p: VariantProps) => ReactNode;
}> = [
  {
    key: "A",
    title: "A. Top bar (current, cleaned)",
    description: "Existing chrome with tags removed. Most familiar; bold accent.",
    Variant: VariantA,
  },
  {
    key: "B",
    title: "B. Side accent",
    description: "3px colored left border, no top bar. Quietest — closer to Blender / TouchDesigner style graph editors.",
    Variant: VariantB,
  },
  {
    key: "C",
    title: "C. Icon tile",
    description: "Small colored square with the kind initial. Information-dense; works well when many node types coexist.",
    Variant: VariantC,
  },
  {
    key: "D",
    title: "D. Pill chip",
    description: "Small uppercase pill in the kind color. Compact, modern, low-chrome.",
    Variant: VariantD,
  },
  {
    key: "E",
    title: "E. Gradient + status dot",
    description: "Subtle gradient header + glowing dot indicator. Most decorative; reads as 'card'.",
    Variant: VariantE,
  },
];

function Showcase() {
  return (
    <div className="min-h-screen bg-[#0e1116] p-6 text-white">
      <h1 className="text-lg font-semibold mb-1">Node design proposals</h1>
      <p className="text-xs text-gray-400 mb-6">
        Each row is one design variant; each column is the same node kind so chrome can be compared directly.
        Format pills (Hz / ch / bit) are removed everywhere — that information now lives on the handles and edges.
      </p>

      <div className="flex flex-col gap-8">
        {VARIANTS.map(({ key, title, description, Variant }) => (
          <section key={key}>
            <div className="mb-2">
              <div className="text-sm font-semibold text-white">{title}</div>
              <div className="text-xs text-gray-400">{description}</div>
            </div>
            <div className="flex flex-row gap-6 items-start flex-wrap">
              <Variant accent="#f87171" title="Audio Input" rightHandle>
                <MockSelect placeholder="Default Microphone" />
              </Variant>
              <Variant accent="#f87171" title="Audio Output" leftHandle>
                <MockSelect placeholder="Speakers (Realtek)" />
              </Variant>
              <Variant accent="#fb923c" accent2="#f59e0b" title="Mixer" rightHandle>
                <MultiHandleRows accent="#fb923c" names={["A", "B"]} />
              </Variant>
              <Variant accent="#a78bfa" title="Virtual Mic" leftHandle>
                <MockSelect placeholder="Cable Mic A" />
              </Variant>
              <Variant accent="#2dd4bf" title="Virtual Speaker" rightHandle>
                <MockSelect placeholder="Cable Speaker A" />
              </Variant>
              <Variant accent="#a855f7" title="Spectrum" leftHandle rightHandle>
                <div className="w-44 h-16 rounded bg-gray-900" />
              </Variant>
            </div>
          </section>
        ))}
      </div>
    </div>
  );
}

const meta: Meta<typeof Showcase> = {
  title: "Design/Node Proposals",
  component: Showcase,
  parameters: { layout: "fullscreen" },
};
export default meta;

type Story = StoryObj<typeof Showcase>;
export const All: Story = {};
