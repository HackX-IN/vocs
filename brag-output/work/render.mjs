// Drives the composition in headless Chromium. Usage:
//   node render.mjs stills 0.5 2.6 ...      -> work/stills/t_<t>.png
//   node render.mjs frames [workers] [from] [to] -> work/frames/f_<n>.jpg (3 sub-frames per frame)
//   node render.mjs timeline                -> work/timeline.json
import { createRequire } from "node:module";
const { chromium } = createRequire(import.meta.url)("playwright");
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const url = pathToFileURL(join(here, "composition", "index.html")).href;
const [mode, ...args] = process.argv.slice(2);
const FPS = 30, DUR = 25, SUB = 3, SHUTTER = 0.5;

async function openPage(browser) {
  const page = await browser.newPage({ viewport: { width: 1920, height: 1080 }, deviceScaleFactor: 1 });
  page.on("console", (m) => { if (m.type() === "error") console.error("page:", m.text()); });
  page.on("pageerror", (e) => console.error("pageerror:", e.message));
  await page.goto(url);
  await page.evaluate(() => window.__ready);
  return page;
}

const browser = await chromium.launch({ args: ["--disable-web-security", "--allow-file-access-from-files"] });
if (mode === "stills") {
  const page = await openPage(browser);
  mkdirSync(join(here, "stills"), { recursive: true });
  for (const a of args) {
    const t = Number(a);
    await page.evaluate((tt) => window.renderFrame(tt), t);
    await page.screenshot({ path: join(here, "stills", `t_${t.toFixed(2)}.png`) });
  }
} else if (mode === "timeline") {
  const page = await openPage(browser);
  const tl = await page.evaluate(() => window.TIMELINE);
  writeFileSync(join(here, "timeline.json"), JSON.stringify(tl, null, 1));
} else if (mode === "frames") {
  mkdirSync(join(here, "frames"), { recursive: true });
  const total = FPS * DUR;
  const workers = Number(args[0] ?? 4);
  const jobs = [];
  const from = Number(args[1] ?? 0), to = Number(args[2] ?? total);
  for (let f = from; f < to; f++) for (let s = 0; s < SUB; s++) jobs.push({ f, s, t: (f + (s / SUB) * SHUTTER) / FPS });
  let next = 0, done = 0;
  const started = Date.now();
  await Promise.all(Array.from({ length: workers }, async () => {
    const page = await openPage(browser);
    while (next < jobs.length) {
      const j = jobs[next++];
      await page.evaluate((tt) => window.renderFrame(tt), j.t);
      await page.screenshot({ path: join(here, "frames", `f_${String(j.f * SUB + j.s).padStart(5, "0")}.jpg`), type: "jpeg", quality: 95 });
      if (++done % 150 === 0) console.log(`${done}/${jobs.length} ${((Date.now() - started) / 1000).toFixed(0)}s`);
    }
  }));
}
await browser.close();
