// Tiny MCP HTTP client to talk to @storybook/addon-mcp on localhost:6006/mcp
import { randomUUID } from "node:crypto";

const URL = "http://localhost:6006/mcp";
let sessionId = null;
let nextId = 1;

async function rpc(method, params = {}) {
  const headers = {
    "Content-Type": "application/json",
    Accept: "application/json, text/event-stream",
  };
  if (sessionId) headers["mcp-session-id"] = sessionId;

  const body = JSON.stringify({ jsonrpc: "2.0", id: nextId++, method, params });
  const res = await fetch(URL, { method: "POST", headers, body });
  const sid = res.headers.get("mcp-session-id");
  if (sid && !sessionId) sessionId = sid;

  const text = await res.text();
  // Parse SSE: lines like "data: {...}"
  const lines = text.split("\n").filter((l) => l.startsWith("data:"));
  if (lines.length === 0) return { raw: text };
  return JSON.parse(lines[0].slice(5).trim());
}

async function notify(method, params = {}) {
  const headers = {
    "Content-Type": "application/json",
    Accept: "application/json, text/event-stream",
  };
  if (sessionId) headers["mcp-session-id"] = sessionId;
  await fetch(URL, {
    method: "POST",
    headers,
    body: JSON.stringify({ jsonrpc: "2.0", method, params }),
  });
}

async function main() {
  const cmd = process.argv[2];
  const arg = process.argv[3];

  await rpc("initialize", {
    protocolVersion: "2024-11-05",
    capabilities: {},
    clientInfo: { name: "cable-cli", version: "1.0" },
  });
  await notify("notifications/initialized");

  if (cmd === "list") {
    const r = await rpc("tools/list");
    console.log(JSON.stringify(r.result?.tools?.map((t) => ({ name: t.name, desc: t.description })), null, 2));
  } else if (cmd === "call") {
    const args = arg ? JSON.parse(arg) : {};
    const r = await rpc("tools/call", { name: process.argv[3], arguments: JSON.parse(process.argv[4] || "{}") });
    console.log(JSON.stringify(r, null, 2));
  } else if (cmd === "list-docs") {
    const r = await rpc("tools/call", { name: "list-all-documentation", arguments: { withStoryIds: true } });
    console.log(JSON.stringify(r, null, 2));
  } else if (cmd === "preview") {
    const ids = process.argv.slice(3);
    const r = await rpc("tools/call", { name: "preview-stories", arguments: { storyIds: ids } });
    console.log(JSON.stringify(r, null, 2));
  } else if (cmd === "doc") {
    const r = await rpc("tools/call", { name: "get-documentation", arguments: { id: process.argv[3] } });
    console.log(JSON.stringify(r, null, 2));
  } else if (cmd === "instructions") {
    const r = await rpc("tools/call", { name: "get-storybook-story-instructions", arguments: {} });
    console.log(JSON.stringify(r, null, 2));
  } else {
    console.error("usage: node mcp.mjs <list|list-docs|preview|doc|instructions|call> [args...]");
  }
}

main().catch((e) => { console.error(e); process.exit(1); });
