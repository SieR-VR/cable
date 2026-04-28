// Capture Storybook story screenshots via Playwright for visual inspection.
import { chromium } from "playwright";
import { mkdir } from "node:fs/promises";
import { join } from "node:path";

const STORY_IDS = [
  "audio-audioedge--stereo-48-k-24",
  "audio-audioedge--mono-44-k-16",
  "audio-audioedge--quad-96-k-24",
  "audio-audioedge--six-ch-96-k-32",
  "audio-audioedge--eight-ch-192-k-32",
  "audio-audioedge--vertical",
  "audio-audiohandle--stereo-48-k-24",
  "audio-audiohandle--quad-96-k-24",
  "audio-audiohandle--six-ch-96-k-32",
  "audio-audiohandle--eight-ch-192-k-32",
  "audio-audiohandle--disabled",
  "audio-audiohandle--connected",
];

const OUT_DIR = process.argv[2] || "C:\\Users\\nwh63\\cable\\storybook-shots";

async function main() {
  await mkdir(OUT_DIR, { recursive: true });
  const browser = await chromium.launch();
  const page = await browser.newPage({ viewport: { width: 900, height: 600 }, deviceScaleFactor: 2 });

  for (const id of STORY_IDS) {
    const url = `http://localhost:6006/iframe.html?id=${id}&viewMode=story`;
    await page.goto(url, { waitUntil: "networkidle" });
    await page.waitForTimeout(500); // settle animations / SVG paths
    const out = join(OUT_DIR, `${id}.png`);
    await page.screenshot({ path: out, fullPage: false });
    console.log("saved", out);
  }

  await browser.close();
}

main().catch((e) => { console.error(e); process.exit(1); });
