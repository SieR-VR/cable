import { chromium } from "playwright";

const url = "http://localhost:6006/iframe.html?id=audio-audioedge--stereo-48-k-24&viewMode=story";

const b = await chromium.launch();
const p = await b.newPage();
await p.goto(url);
await p.waitForTimeout(3000);

const data = await p.evaluate(() => {
  const handle = document.querySelector(".react-flow__handle.source");
  const dots = Array.from(document.querySelectorAll(".react-flow__handle.source span"));
  const paths = Array.from(document.querySelectorAll('svg path[stroke="#3fb950"]'));
  const handleRect = handle.getBoundingClientRect();
  const handleCx = handleRect.left + handleRect.width / 2;
  const handleCy = handleRect.top + handleRect.height / 2;
  const dotPositions = dots.map((d) => {
    const r = d.getBoundingClientRect();
    return { cx: r.left + r.width / 2, cy: r.top + r.height / 2, w: r.width };
  });
  const strandStarts = paths.map((path) => {
    const svg = path.ownerSVGElement;
    const m = /^M\s*([-\d.]+)[, ]([-\d.]+)/.exec(path.getAttribute("d"));
    const pt = svg.createSVGPoint();
    pt.x = +m[1];
    pt.y = +m[2];
    const screen = pt.matrixTransform(path.getScreenCTM());
    return { x: screen.x, y: screen.y, internalX: +m[1], internalY: +m[2] };
  });
  // Inspect transforms on path ancestors
  const ancestors = [];
  let el = paths[0];
  while (el && el !== document.body) {
    const cs = getComputedStyle(el);
    ancestors.push({
      tag: el.tagName + (el.className?.baseVal ? "." + el.className.baseVal : ""),
      transform: cs.transform === "none" ? null : cs.transform,
      svgTransform: el.getAttribute && el.getAttribute("transform"),
      viewBox: el.getAttribute && el.getAttribute("viewBox"),
      svgAttrs: el.tagName === "svg" ? Array.from(el.attributes).map(a => a.name + "=" + a.value).join(", ") : null,
      position: cs.position,
      left: cs.left,
      top: cs.top,
      bbox: el.getBoundingClientRect ? (() => { const r = el.getBoundingClientRect(); return { l: r.left, t: r.top, w: r.width, h: r.height }; })() : null,
    });
    el = el.parentElement;
  }
  return { handleCx, handleCy, handleW: handleRect.width, dotPositions, strandStarts, ancestors };
});
console.log(JSON.stringify(data, null, 2));
await b.close();
