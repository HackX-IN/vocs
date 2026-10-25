/* Motion kit for the Vocs film. Every frame is a pure function of time, so the
   same t always draws the same pixels (needed for sub-frame motion blur). */
"use strict";

const BPM = 120;
const BEAT = 60 / BPM; // 0.5s = 15 frames at 30fps

const clamp = (x, a = 0, b = 1) => Math.min(b, Math.max(a, x));
const lerp = (a, b, t) => a + (b - a) * t;
const prog = (t, a, b) => clamp((t - a) / (b - a));
const easeOutCubic = (x) => 1 - Math.pow(1 - x, 3);
const easeOutQuart = (x) => 1 - Math.pow(1 - x, 4);
const easeOutExpo = (x) => (x >= 1 ? 1 : 1 - Math.pow(2, -10 * x));
const easeInCubic = (x) => x * x * x;
const easeInExpo = (x) => (x <= 0 ? 0 : Math.pow(2, 10 * x - 10));
const easeInOutCubic = (x) => (x < 0.5 ? 4 * x * x * x : 1 - Math.pow(-2 * x + 2, 3) / 2);
const easeInOutExpo = (x) =>
  x <= 0 ? 0 : x >= 1 ? 1 : x < 0.5 ? Math.pow(2, 20 * x - 10) / 2 : (2 - Math.pow(2, -20 * x + 10)) / 2;

/* --motion-ease-standard: cubic-bezier(0.32, 0.72, 0, 1) */
function cubicBezier(p1x, p1y, p2x, p2y) {
  const cx = 3 * p1x, bx = 3 * (p2x - p1x) - cx, ax = 1 - cx - bx;
  const cy = 3 * p1y, by = 3 * (p2y - p1y) - cy, ay = 1 - cy - by;
  const sx = (t) => ((ax * t + bx) * t + cx) * t;
  const sy = (t) => ((ay * t + by) * t + cy) * t;
  const dx = (t) => (3 * ax * t + 2 * bx) * t + cx;
  return (x) => {
    if (x <= 0) return 0;
    if (x >= 1) return 1;
    let t = x;
    for (let i = 0; i < 8; i++) {
      const e = sx(t) - x;
      const d = dx(t);
      if (Math.abs(e) < 1e-6 || Math.abs(d) < 1e-6) break;
      t -= e / d;
    }
    return sy(clamp(t));
  };
}
const easeStd = cubicBezier(0.32, 0.72, 0, 1);

/* A damped spring from 0 to 1 at rest. Defaults are --spring-state (300/25). */
function spring(t, k = 300, c = 25) {
  if (t <= 0) return 0;
  const w0 = Math.sqrt(k);
  const z = c / (2 * Math.sqrt(k));
  if (z < 1) {
    const wd = w0 * Math.sqrt(1 - z * z);
    return 1 - Math.exp(-z * w0 * t) * (Math.cos(wd * t) + ((z * w0) / wd) * Math.sin(wd * t));
  }
  return 1 - Math.exp(-w0 * t) * (1 + w0 * t);
}
const panel = (t) => spring(t, 260, 26);
const bouncy = (t) => spring(t, 420, 18);

function mulberry32(seed) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const $ = (id) => document.getElementById(id);
function show(el, on) { el.style.visibility = on ? "visible" : "hidden"; }
function css(el, props) { for (const k in props) el.style[k] = props[k]; }

/* The identity mark (src/components/global/mark/Mark.tsx), same paths. */
const QUOTE = "M0 0H38C72 0 100 28 100 64V120H6V62H30C52 62 52 38 30 38H2Z";
const HEAD = "M-6 46.5L0 45.5C12 39.5 30 26.5 47 26C63 25.5 82 36 87.4 52.5C90 60 90.8 75 91 105H-6Z";
let markSeq = 0;
function markSVG(extraClass = "") {
  const id = `m${markSeq++}`;
  return `<svg class="mark ${extraClass}" viewBox="0 24.5 91.2 75.5" fill="currentColor">
    <defs>
      <clipPath id="${id}t"><rect x="0" y="0" width="91.2" height="100" rx="22.4"/></clipPath>
      <path id="${id}q" d="${QUOTE}"/>
      <mask id="${id}h" maskUnits="userSpaceOnUse" x="0" y="0" width="100" height="100">
        <rect width="100" height="100" fill="#fff"/>
        <g fill="#000" class="eyes">
          <use href="#${id}q" transform="translate(44.3 43.2) rotate(-4) scale(0.126)"/>
          <use href="#${id}q" transform="translate(60.9 42) rotate(-4) scale(0.126)"/>
        </g>
      </mask>
    </defs>
    <g clip-path="url(#${id}t)"><path class="head" mask="url(#${id}h)" d="${HEAD}"/></g>
  </svg>`;
}

/* The app icon's artwork (assets/icon.svg) in its normalised 0..100 space:
   ink head plus white quotes, drawn over the orange squircle. */
function iconArtSVG() {
  return `<svg viewBox="0 0 100 100" preserveAspectRatio="none">
    <defs>
      <linearGradient id="inkg" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#232323"/><stop offset="1" stop-color="#090909"/></linearGradient>
      <path id="iq" d="${QUOTE}"/>
    </defs>
    <g id="ihead"><path fill="url(#inkg)" d="${HEAD}"/></g>
    <g id="iquotes" fill="#fff">
      <g id="iq1"><use href="#iq" transform="translate(44.3 43.2) rotate(-4) scale(0.126)"/></g>
      <g id="iq2"><use href="#iq" transform="translate(60.9 42) rotate(-4) scale(0.126)"/></g>
    </g>
  </svg>`;
}

/* PillBars (src/app/pill/_components/PillBars.tsx) re-run deterministically:
   the same rest shape, flank weights, attack/release and idle ripple, stepped
   at 60Hz from a level function so any t reproduces the same bar heights. */
const REST_SHAPE = [0.3, 0.5, 0.38];
const FLANK_WEIGHT = [0.7, 0.85];
/* Bars at time t: the 60Hz loop value at the step before t, eased to the next. */
function barsSmooth(t, levelFn) {
  const dt = 1 / 60;
  const a = barsStep(Math.floor(t / dt) * dt, levelFn);
  const b = barsStep((Math.floor(t / dt) + 1) * dt, levelFn);
  const f = (t - Math.floor(t / dt) * dt) / dt;
  return a.map((v, i) => lerp(v, b[i], f));
}
function barsStep(t, levelFn) {
  const dt = 1 / 60;
  const n = Math.round(t / dt);
  const start = Math.max(0, n - 180);
  const shown = [...REST_SHAPE];
  let last = 0, beforeLast = 0;
  for (let i = start; i <= n; i++) {
    const tt = i * dt;
    const level = clamp(levelFn(tt));
    const target = [
      Math.max(REST_SHAPE[0], last * FLANK_WEIGHT[0]),
      Math.max(REST_SHAPE[1], level),
      Math.max(REST_SHAPE[2], beforeLast * FLANK_WEIGHT[1]),
    ];
    beforeLast = last;
    last = level;
    for (let b = 0; b < 3; b++) {
      const phase = (tt * 1000 / 1600 + b / 3) * Math.PI * 2;
      const resting = REST_SHAPE[b] + 0.08 * Math.sin(phase);
      const wanted = Math.max(resting, target[b]);
      shown[b] += (wanted - shown[b]) * (wanted > shown[b] ? 0.6 : 0.14);
    }
  }
  return shown;
}

/* Sample a string's ink into points: the dot-matrix vocabulary of the brand. */
function samplePoints(draw, step, w = 1920, h = 1080) {
  const c = document.createElement("canvas");
  c.width = w;
  c.height = h;
  const ctx = c.getContext("2d");
  ctx.fillStyle = "#fff";
  draw(ctx);
  const data = ctx.getImageData(0, 0, w, h).data;
  const pts = [];
  for (let y = 0; y < h; y += step) {
    for (let x = 0; x < w; x += step) {
      if (data[(y * w + x) * 4 + 3] > 128) pts.push([x, y]);
    }
  }
  return pts;
}
