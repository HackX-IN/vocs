/* The Vocs film: eight scenes driven by one clock. render(t) is pure in t. */
"use strict";

const W = 1920, H = 1080;
const T = {
  typeA: 0.2, typeB: 1.5, keysIn: 1.85, press: 2.5, pill: 3.0,
  slide: 3.5, tag: 4.3, s3: 5.5, press1: 5.75, speakA: 6.3, speakB: 8.7, press2: 9.0, paste: 9.12,
  wipe1: 11.0, s4: 11.0, cards: [11.5, 12.0, 12.5, 13.0], zoom: 14.0,
  s5a: 14.0, s5b: 16.0, esc1: 16.5, esc2: 17.5, wipe2: 18.5, s6: 18.5, chips: [18.75, 19.0, 19.25, 19.5], h6: 19.5,
  s7: 21.5, flood: 21.5, land: 22.0, end: 25.0,
};
const TIMELINE = { keys: [], ticks: [], whoosh: [], hits: [], chimes: [], presses: [], filter: [] };

const ACCENT = [255, 106, 43];
const INK_D = [246, 242, 235];
const INK_L = [27, 26, 23];

/* ───────────────────────── build ───────────────────────── */
const el = {};
const S1_TEXT = "You think faster than you type.";
const CAPTION = "Push the launch to Thursday. Design needs one more pass on the onboarding screens.";
const CARDS = [
  { cls: "light", app: "Slack", glyph: "#", gbg: "#efe9e1", gfg: "#1b1a17", meta: "#launch", line: "Shipping Thursday. Notes are in the doc." },
  { cls: "dark", app: "VS Code", glyph: "{ }", gbg: "#2b2926", gfg: "#ff6a2b", meta: "search.ts", line: "// debounce the search to 180ms" },
  { cls: "term", app: "Terminal", glyph: ">_", gbg: "#1f1d1a", gfg: "#7bc83b", meta: "~/vocs", line: '$ claude "split settings into sections"' },
  { cls: "notes", app: "Notes", glyph: "✎", gbg: "#f1e6c8", gfg: "#1b1a17", meta: "Today", line: "Oat milk, lemons, coffee." },
];
const GREETINGS = ["Hello", "नमस्ते", "مرحبا", "Hola", "こんにちは", "Bonjour", "안녕하세요", "Ciao", "Привет", "Olá", "你好", "Hallo", "Hello"];
const CHIPS = ["No account", "No subscription", "No word cap", "No telemetry"];
const BAN = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round"><circle cx="12" cy="12" r="10"/><path d="m4.9 4.9 14.2 14.2"/></svg>';

function charSpans(host, text) {
  host.innerHTML = "";
  return [...text].map((c) => {
    const s = document.createElement("span");
    s.className = "ch";
    s.textContent = c;
    host.appendChild(s);
    return s;
  });
}

let s1Chars, s1Times, burst, lattice, wordDots, wordBox, words, syllables, cardEls, h4Chars, h6Chars, chipEls, rollEls, grainUrls;

function build() {
  for (const id of ["world", "vignette", "glow", "paperbg", "lattice", "fx", "wipe", "grain", "pillwrap", "pill", "pillflash",
    "s1", "s1text", "keyopt", "keyspace", "s2", "s2tag", "s3", "st1", "st2", "st3", "mail", "mailwords", "mailhl", "mailcaret",
    "caption", "captext", "kc1", "s4", "h4", "deck", "ticker", "s5a", "big99", "langlabel", "langsub", "roll",
    "s5b", "esckey", "escline1", "escline2", "s6", "chips", "h6", "s7", "flood", "icon-shadow", "wordmark", "tagline", "meta"]) {
    el[id] = $(id);
  }
  el.lctx = el.lattice.getContext("2d");
  el.fctx = el.fx.getContext("2d");
  el.wctx = el.wipe.getContext("2d");

  /* Pill: mark + three bars + countdown line, at its real 104x52 size. */
  el.pill.innerHTML = `${markSVG()}<div class="bars"><i></i><i></i><i></i></div><div class="line"><b></b></div>`;
  el.pillMark = el.pill.querySelector(".mark");
  el.pillBars = [...el.pill.querySelectorAll(".bars i")];
  el.pillBarsBox = el.pill.querySelector(".bars");
  el.pillLine = el.pill.querySelector(".line");
  el.pillLineFill = el.pill.querySelector(".line b");

  /* S1 typed sentence and human typing rhythm. */
  s1Chars = charSpans(el.s1text, S1_TEXT);
  const fStart = S1_TEXT.indexOf("faster");
  s1Chars.forEach((s, i) => { if (i >= fStart && i < fStart + 6) s.dataset.f = "1"; });
  const rnd = mulberry32(7);
  const wts = [...S1_TEXT].map((c, i) => (c === " " ? 1.7 : 0.7 + rnd() * 0.9) + (i > 0 && S1_TEXT[i - 1] === " " ? 0.4 : 0));
  const total = wts.reduce((a, b) => a + b, 0);
  let acc = 0;
  s1Times = wts.map((w) => { const at = T.typeA + (acc / total) * (T.typeB - T.typeA); acc += w; return at; });
  s1Times.forEach((at, i) => { if (S1_TEXT[i] !== " ") TIMELINE.keys.push(at); });

  /* Burst particles sampled from the sentence as laid out after it lifts. */
  const lift = -110;
  const rects = s1Chars.map((s) => s.getBoundingClientRect());
  const fontPx = 92;
  const pts = [];
  s1Chars.forEach((s, i) => {
    if (S1_TEXT[i] === " ") return;
    const r = rects[i];
    const local = samplePoints((ctx) => {
      ctx.font = `600 ${fontPx}px "Inter Display"`;
      const m = ctx.measureText(S1_TEXT[i]);
      ctx.fillText(S1_TEXT[i], r.left, r.top + lift + m.fontBoundingBoxAscent);
    }, 4);
    local.forEach((p) => pts.push([p[0], p[1], s.dataset.f === "1"]));
  });
  const pr = mulberry32(11);
  burst = pts.map(([x, y, f]) => {
    const ang = Math.atan2(y - 380, x - 960) + (pr() - 0.5) * 1.6;
    const dist = 70 + pr() * 300;
    const tr = Math.sqrt(pr());
    const ta = pr() * Math.PI * 2;
    return { x, y, f, dx: Math.cos(ang) * dist, dy: Math.sin(ang) * dist * 0.8 - 60 * pr(), tx: 960 + Math.cos(ta) * 200 * tr, ty: 540 + Math.sin(ta) * 90 * tr, spin: (pr() - 0.5) * 3.2, size: 1.6 + pr() * 1.6, delay: pr() * 0.06 };
  });

  /* Background lattice (--dot-grid-pitch 14px at 2x). */
  lattice = [];
  for (let y = 14; y < H; y += 28) for (let x = 14; x < W; x += 28) lattice.push([x, y]);

  /* S2 dot-matrix wordmark. */
  const pitch = 15;
  const ox = 845, base = 600;
  const lit = samplePoints((ctx) => { ctx.font = '900 330px "Inter Display"'; ctx.fillText("Vocs", ox, base); }, pitch);
  const litSet = new Set(lit.map((p) => p.join(",")));
  let minX = 1e9, maxX = -1e9, minY = 1e9, maxY = -1e9;
  lit.forEach(([x, y]) => { minX = Math.min(minX, x); maxX = Math.max(maxX, x); minY = Math.min(minY, y); maxY = Math.max(maxY, y); });
  wordBox = { minX, maxX, minY, maxY };
  const wr = mulberry32(23);
  wordDots = [];
  for (let y = minY - pitch; y <= maxY + pitch; y += pitch) {
    for (let x = minX - pitch; x <= maxX + pitch; x += pitch) {
      const on = litSet.has(`${x},${y}`);
      wordDots.push({ x, y, on, at: 3.62 + 0.55 * ((x - minX) / (maxX - minX)) + wr() * 0.1, bend: (wr() - 0.5) * 380, fall: wr() });
    }
  }
  css(el.s2tag, { left: `${minX}px`, top: `${maxY + 46}px` });

  /* S3 caption words and the syllable envelope the bars follow. */
  words = CAPTION.split(" ");
  const ww = words.map((w) => w.length + 2);
  const wsum = ww.reduce((a, b) => a + b, 0);
  let wacc = 0;
  const sr = mulberry32(31);
  syllables = [];
  words = words.map((w, i) => {
    const a = T.speakA + (wacc / wsum) * (T.speakB - T.speakA);
    wacc += ww[i];
    const b = T.speakA + (wacc / wsum) * (T.speakB - T.speakA);
    const n = Math.max(1, (w.toLowerCase().match(/[aeiouy]+/g) || []).length);
    for (let k = 0; k < n; k++) syllables.push({ c: a + ((k + 0.5) / n) * (b - a) * 0.85, amp: 0.6 + sr() * 0.4 });
    return { w, a, b };
  });

  /* S4 cards. */
  cardEls = CARDS.map((c) => {
    const d = document.createElement("div");
    d.className = `card ${c.cls}`;
    d.innerHTML = `<div class="bar"><div class="dots"><u style="background:#ff5f57"></u><u style="background:#febc2e"></u><u style="background:#28c840"></u></div>
      <div class="glyph" style="background:${c.gbg};color:${c.gfg}">${c.glyph}</div>${c.app}</div>
      <div class="bd"><div class="meta">${c.meta}</div><span class="ln"><span class="hl"></span><span class="tx"></span></span><span class="caret"></span></div>`;
    el.deck.appendChild(d);
    return { root: d, tx: d.querySelector(".tx"), hl: d.querySelector(".hl"), caret: d.querySelector(".caret"), line: c.line };
  });
  h4Chars = charSpans(el.h4, "Works wherever you can type.");
  const names = ["Slack", "Teams", "Notes", "Chrome", "VS Code", "Terminal", "Mail", "Everything else"];
  el.ticker.innerHTML = new Array(4).fill(names.map((n) => `<b>${n}</b>`).join("  ·  ")).join("  ·  ");

  /* S5a. */
  css(el.big99, { left: "170px", top: "300px" });
  css(el.langlabel, { left: "190px", top: "740px" });
  css(el.langsub, { left: "192px", top: "828px" });
  css(el.roll, { left: "1040px", top: "380px" });
  rollEls = GREETINGS.map((g) => {
    const d = document.createElement("div");
    d.className = "item";
    d.textContent = g;
    el.roll.appendChild(d);
    return d;
  });

  /* S5b. */
  css(el.esckey, { left: "430px", top: "355px" });
  css(el.escline1, { top: "630px" });
  css(el.escline2, { top: "740px" });

  /* S6 chips, laid out centred on one row. */
  chipEls = CHIPS.map((c) => {
    const d = document.createElement("div");
    d.className = "nochip";
    d.innerHTML = `${BAN}<span>${c}</span>`;
    el.chips.appendChild(d);
    return d;
  });
  const widths = chipEls.map((d) => d.offsetWidth);
  const gap = 26;
  let cx = (W - (widths.reduce((a, b) => a + b, 0) + gap * (widths.length - 1))) / 2;
  chipEls.forEach((d, i) => { css(d, { left: `${cx}px`, top: "390px" }); cx += widths[i] + gap; });
  h6Chars = charSpans(el.h6, "Nothing leaves your Mac.");

  /* S7 icon art. */
  el.flood.innerHTML = iconArtSVG();
  el.ihead = $("ihead");
  el.iquotes = $("iquotes");
  el.iq1 = $("iq1");
  el.iq2 = $("iq2");
  css(el.wordmark, { top: "535px" });
  css(el.tagline, { top: "722px" });
  css(el.meta, { top: "830px" });

  /* Film grain: a few deterministic noise plates, swapped at 24Hz. */
  grainUrls = [];
  const gr = mulberry32(99);
  for (let k = 0; k < 6; k++) {
    const c = document.createElement("canvas");
    c.width = 960; c.height = 540;
    const ctx = c.getContext("2d");
    const img = ctx.createImageData(960, 540);
    for (let i = 0; i < img.data.length; i += 4) {
      const v = Math.floor(gr() * 255);
      img.data[i] = img.data[i + 1] = img.data[i + 2] = v;
      img.data[i + 3] = 255;
    }
    ctx.putImageData(img, 0, 0);
    grainUrls.push(`url(${c.toDataURL()})`);
  }

  /* Audio cues that are fixed by the edit. */
  TIMELINE.hits.push({ t: T.press, kind: "press" }, { t: T.pill, kind: "drop" }, { t: T.s5b, kind: "pill" }, { t: T.land, kind: "final" });
  TIMELINE.presses.push(T.press1 - 0.08, T.press2, T.esc1, T.esc2);
  TIMELINE.chimes.push({ t: T.paste, kind: "paste" });
  TIMELINE.whoosh.push(T.slide - 0.05, T.s3 - 0.1, T.wipe1 - 0.25, ...T.cards.map((c) => c - 0.12), T.zoom - 0.2, T.wipe2 - 0.25, T.flood - 0.05);
  T.chips.forEach((c) => TIMELINE.ticks.push({ t: c, kind: "chip" }));
  T.cards.forEach((c) => TIMELINE.ticks.push({ t: c + 0.18, kind: "paste" }));
  TIMELINE.filter.push({ a: T.esc1, b: T.esc2, c: T.esc2 + 0.24 });
  for (let k = 1; k < GREETINGS.length; k++) {
    const at = rollTimeFor(k - 0.5);
    if (at) TIMELINE.ticks.push({ t: at, kind: "roll" });
  }
  TIMELINE.words = words.map((w) => w.a);
}

/* ───────────────────────── signals ───────────────────────── */
const hash = (x) => { const s = Math.sin(x * 127.1 + 311.7) * 43758.5453; return s - Math.floor(s); };
function musicLevel(tt) {
  const ph = ((tt % BEAT) + BEAT) % BEAT;
  const off = (((tt + BEAT / 2) % BEAT) + BEAT) % BEAT;
  return 0.22 + 0.75 * Math.exp(-ph / 0.1) + 0.32 * Math.exp(-off / 0.05) + 0.12 * hash(Math.floor(tt * 30));
}
function speechLevel(tt) {
  let v = 0;
  for (const s of syllables) {
    const d = (tt - s.c) / 0.055;
    if (Math.abs(d) < 4) v = Math.max(v, s.amp * Math.exp(-d * d));
  }
  return v + 0.05 * hash(Math.floor(tt * 40));
}
const rollPos = (t) => (GREETINGS.length - 1) * easeOutQuart(prog(t, 14.1, 15.5));
function rollTimeFor(pos) {
  for (let t = 14.1; t < 15.5; t += 1 / 240) if (rollPos(t) >= pos) return t;
  return null;
}

/* ───────────────────────── scenes ───────────────────────── */
function camera(t) {
  let s = 1, x = 0, y = 0, blur = 0, op = 1;
  if (t < T.pill) s = 1 + 0.045 * easeInOutCubic(prog(t, 0, T.press)) - 0.03 * spring(t - T.press, 500, 30) * (t > T.press ? 1 : 0);
  else if (t < T.s3) s = 1.0 + 0.02 * prog(t, T.pill, T.s3);
  else if (t < T.s4) s = 1.0 + 0.03 * easeInOutCubic(prog(t, T.s3, T.s4)) + 0.012 * Math.exp(-Math.max(0, t - T.paste) / 0.25) * (t > T.paste ? 1 : 0);
  else if (t < T.zoom) {
    s = 1.0 + 0.02 * prog(t, T.s4, T.zoom - 0.2) + 0.5 * easeInExpo(prog(t, T.zoom - 0.2, T.zoom));
    blur = 14 * easeInCubic(prog(t, T.zoom - 0.15, T.zoom));
  } else if (t < T.s5b) {
    const k = prog(t, T.zoom, T.zoom + 0.35);
    s = lerp(0.82, 1, easeOutExpo(k)) + 0.02 * prog(t, T.zoom + 0.35, T.s5b);
    blur = 14 * (1 - easeOutCubic(k));
  } else if (t < T.s6) s = 1.0 + 0.025 * prog(t, T.s5b, T.s6);
  else if (t < T.s7) s = 1.0 + 0.02 * prog(t, T.s6, T.s7);
  else s = 1.0 + 0.035 * easeInOutCubic(prog(t, T.land, T.end));
  // Beat punches on the three biggest hits.
  for (const h of [T.pill, T.s5b, T.land]) if (t >= h) s += 0.018 * Math.exp(-(t - h) / 0.12) * Math.cos((t - h) * 30);
  css(el.world, { transform: `translate(${x}px, ${y}px) scale(${s})`, filter: blur > 0.05 ? `blur(${blur}px)` : "none", opacity: op });
}

function backgrounds(t) {
  const paper = t >= T.wipe2 && t < T.flood + 0.25 ? 1 : 0;
  el.paperbg.style.opacity = paper;
  el.vignette.style.opacity = paper ? 0.25 : 1;
  el.glow.style.transform = `translate(${Math.sin(t * 0.5) * 30}px, ${Math.cos(t * 0.4) * 20}px)`;
  el.grain.style.backgroundImage = grainUrls[Math.floor(t * 24) % grainUrls.length];
}

const RIPPLES = [
  { t0: T.press, x: 960, y: 795, speed: 1500, w: 110, amp: 1.0 },
  { t0: T.pill, x: 960, y: 540, speed: 1700, w: 120, amp: 1.0 },
  { t0: T.paste, x: 1260, y: 330, speed: 1600, w: 90, amp: 0.6 },
  { t0: T.s5b, x: 960, y: 430, speed: 1700, w: 120, amp: 0.9 },
  { t0: T.esc2, x: 960, y: 430, speed: 1500, w: 90, amp: 0.6 },
  { t0: T.h6, x: 960, y: 670, speed: 1500, w: 110, amp: 0.8 },
  { t0: T.land, x: 960, y: 350, speed: 1500, w: 130, amp: 1.0 },
];
function drawLattice(t) {
  const ctx = el.lctx;
  ctx.clearRect(0, 0, W, H);
  const paper = t >= T.wipe2 && t < T.flood + 0.25;
  const ink = paper ? INK_L : [255, 244, 230];
  const baseA = paper ? 0.11 : 0.085;
  const live = RIPPLES.filter((r) => t >= r.t0 && t < r.t0 + 1.6);
  const speaking = t > T.speakA - 0.2 && t < T.press2 ? speechLevel(t) : 0;
  for (const [x, y] of lattice) {
    let I = 0;
    for (const r of live) {
      const d = Math.hypot(x - r.x, y - r.y);
      const front = (t - r.t0) * r.speed;
      const g = (d - front) / r.w;
      I += r.amp * Math.exp(-g * g) * Math.exp(-(t - r.t0) / 0.9);
    }
    if (speaking > 0) {
      const d = Math.hypot(x - 1260, y - 900);
      I += speaking * 0.55 * Math.exp(-d / 260);
    }
    I = Math.min(1, I);
    const rad = 1.6 + 2.6 * I;
    const c = paper ? [lerp(ink[0], 232, I), lerp(ink[1], 80, I), lerp(ink[2], 26, I)] : [lerp(ink[0], ACCENT[0], I), lerp(ink[1], ACCENT[1], I), lerp(ink[2], ACCENT[2], I)];
    ctx.fillStyle = `rgba(${c[0] | 0},${c[1] | 0},${c[2] | 0},${baseA + I * 0.75})`;
    ctx.beginPath();
    ctx.arc(x, y, rad, 0, Math.PI * 2);
    ctx.fill();
  }
}

function scene1(t) {
  const on = t < T.press + 0.02;
  show(el.s1, t < T.pill + 0.3);
  // The sentence types, lifts as the keys arrive, then turns to dots.
  const lift = -110 * panel(t - T.keysIn);
  css(el.s1text, { transform: `translateY(${lift}px)`, visibility: on ? "visible" : "hidden" });
  let lastTyped = -1;
  const accentK = easeOutCubic(prog(t, T.typeB + 0.15, T.typeB + 0.4));
  s1Chars.forEach((s, i) => {
    const typed = t >= s1Times[i];
    if (typed) lastTyped = i;
    s.style.opacity = typed ? 1 : 0;
    s.style.display = typed || t >= T.press ? "inline" : "none";
    if (s.dataset.f) s.style.color = `rgb(${lerp(246, 255, accentK) | 0},${lerp(242, 106, accentK) | 0},${lerp(235, 43, accentK) | 0})`;
  });
  // Caret: blinks when idle, solid while typing.
  let caret = el.s1caret;
  if (!caret) {
    caret = document.createElement("span");
    caret.className = "caret";
    caret.style.cssText = "position:absolute;width:6px;height:92px;top:22px;background:var(--accent);border-radius:3px;";
    el.s1text.appendChild(caret);
    el.s1caret = caret;
  }
  const typing = t >= T.typeA && t < T.typeB + 0.1;
  const blink = typing || Math.floor(t / 0.5) % 2 === 0;
  const ref = lastTyped >= 0 ? s1Chars[lastTyped] : s1Chars[0];
  const x = lastTyped >= 0 ? ref.offsetLeft + ref.offsetWidth + 6 : ref.offsetLeft - 8;
  css(caret, { left: `${x}px`, opacity: blink && t < T.press ? 1 : 0 });

  // Keycaps: rise, press as a chord, fall away.
  const k = panel(t - T.keysIn);
  const away = easeInCubic(prog(t, T.press + 0.22, T.press + 0.6));
  [[el.keyopt, 580, T.press - 0.04], [el.keyspace, 780, T.press]].forEach(([key, left, pt]) => {
    const dip = t >= pt ? Math.exp(-Math.max(0, t - pt - 0.05) / 0.08) * (t < pt + 0.05 ? (t - pt) / 0.05 : 1) : 0;
    const y = lerp(1180, 712, k) + 12 * dip + 520 * away;
    css(key, {
      left: `${left}px`, top: `${y}px`, opacity: t < T.keysIn ? 0 : 1 - away,
      transform: `perspective(1000px) rotateX(${lerp(48, 20, k)}deg) scale(${1 - 0.03 * dip})`,
      boxShadow: `0 ${14 - 11 * dip}px 0 #0b0a09, 0 ${30 - 14 * dip}px 70px rgba(0,0,0,.55), 0 0 ${120 * dip}px rgba(255,106,43,${0.55 * dip}), inset 0 2px 0 rgba(255,244,230,.08)`,
    });
  });
}

function burstPos(p, t) {
  const u = t - T.press - p.delay;
  if (u <= 0) return [p.x, p.y];
  const e = easeOutExpo(clamp(u / 0.22));
  let x = p.x + p.dx * e, y = p.y + p.dy * e;
  const c = easeInCubic(prog(u, 0.2, T.pill - T.press - p.delay));
  if (c > 0) {
    const ang = p.spin * (1 - c);
    const ox = x - p.tx, oy = y - p.ty;
    const rx = ox * Math.cos(ang) - oy * Math.sin(ang), ry = ox * Math.sin(ang) + oy * Math.cos(ang);
    x = p.tx + rx * (1 - c);
    y = p.ty + ry * (1 - c);
  }
  return [x, y];
}

function drawFx(t) {
  const ctx = el.fctx;
  ctx.clearRect(0, 0, W, H);
  // S1 burst: dots with speed streaks, sucked into where the pill appears.
  if (t >= T.press && t < T.pill + 0.04) {
    const a = 1 - prog(t, T.pill - 0.05, T.pill + 0.03);
    ctx.lineCap = "round";
    for (const p of burst) {
      const [x, y] = burstPos(p, t);
      const [px, py] = burstPos(p, t - 0.011);
      const col = p.f ? "255,106,43" : "246,242,235";
      ctx.strokeStyle = `rgba(${col},${0.95 * a})`;
      ctx.lineWidth = p.size * 2;
      ctx.beginPath();
      ctx.moveTo(px, py);
      ctx.lineTo(x + 0.01, y);
      ctx.stroke();
    }
  }
  // S2 wordmark: dots stream out of the pill into a dot-matrix "Vocs".
  if (t >= 3.5 && t < T.s3 + 0.5) {
    const pillP = pillPose(t);
    const exitK = prog(t, T.s3 - 0.12, T.s3 + 0.22);
    for (const d of wordDots) {
      if (!d.on) {
        const k = easeOutCubic(prog(t, 3.55 + (d.x - wordBox.minX) / 2400, 3.85 + (d.x - wordBox.minX) / 2400));
        const a = 0.09 * k * (1 - exitK);
        if (a <= 0.002) continue;
        ctx.fillStyle = `rgba(246,242,235,${a})`;
        ctx.beginPath();
        ctx.arc(d.x, d.y, 2.4, 0, Math.PI * 2);
        ctx.fill();
        continue;
      }
      const u = t - d.at;
      if (u <= 0) continue;
      const fl = easeOutCubic(clamp(u / 0.42));
      const ox = pillP.x + 110 * pillP.s / 4, oy = pillP.y;
      const mx = (ox + d.x) / 2, my = (oy + d.y) / 2 + d.bend;
      let x = (1 - fl) * (1 - fl) * ox + 2 * (1 - fl) * fl * mx + fl * fl * d.x;
      let y = (1 - fl) * (1 - fl) * oy + 2 * (1 - fl) * fl * my + fl * fl * d.y;
      let r = 6.2 * (0.35 + 0.65 * bouncy(u - 0.3)) * (u < 0.3 ? 0.35 + u : 1);
      let a = 1;
      // Exit: dots drop away, staggered.
      const ek = easeInCubic(clamp(exitK * 1.6 - d.fall * 0.6));
      y += 260 * ek;
      a *= 1 - ek;
      r *= 1 - 0.5 * ek;
      if (a <= 0.01) continue;
      ctx.fillStyle = `rgba(246,242,235,${a})`;
      ctx.beginPath();
      ctx.arc(x, y, Math.max(0.1, r), 0, Math.PI * 2);
      ctx.fill();
    }
  }
}

function scene2(t) {
  show(el.s2, t >= T.pill && t < T.s3 + 0.5);
  const k = panel(t - T.tag);
  const out = easeInCubic(prog(t, T.s3 - 0.25, T.s3 + 0.02));
  css(el.s2tag, { opacity: clamp(k * 1.5) * (1 - out), transform: `translateY(${(1 - k) * 34 - 30 * out}px)`, filter: `blur(${(1 - clamp(k * 1.3)) * 10}px)` });
}

/* Where the pill is, at what scale, and what it shows. */
function pillPose(t) {
  const p = { x: 960, y: 540, s: 4.5, op: 0, mode: "bars", remaining: 1, lift: 0 };
  if (t < T.pill) return p;
  if (t < T.s3) {
    const k = panel(t - T.slide);
    p.x = lerp(960, 560, k);
    p.y = lerp(540, 470, k);
    p.s = lerp(4.5, 4.0, k);
    p.op = 1;
  } else if (t < T.s5b) {
    const k = panel(t - T.s3);
    p.x = lerp(560, 1260, k);
    p.y = lerp(470, 930, k) - 140 * Math.sin(Math.PI * clamp(k));
    p.s = lerp(4.0, 2.0, k);
    p.op = 1;
    // Committed: the window rises --pill-exit-travel and fades over 160ms.
    const e = easeStd(prog(t, T.press2, T.press2 + 0.16));
    p.lift = -8 * p.s * e;
    p.op = 1 - e;
  } else if (t < T.wipe2 + 0.2) {
    p.x = 960; p.y = 430; p.s = 4.5; p.op = 1;
    if (t >= T.esc1 && t < T.esc2) { p.mode = "line"; p.remaining = 1 - (t - T.esc1) / 3; }
    else if (t >= T.esc2 && t < T.esc2 + 0.24) { p.mode = "line"; p.remaining = lerp(1 - (T.esc2 - T.esc1) / 3, 1, easeStd(prog(t, T.esc2, T.esc2 + 0.24))); }
  }
  return p;
}

function drawPill(t) {
  const p = pillPose(t);
  css(el.pillwrap, { opacity: p.op, transform: `translate(${p.x - 52}px, ${p.y - 26 + p.lift}px) scale(${p.s})` });
  if (p.op <= 0) return;
  const speech = t >= T.s3 + 0.3 && t < T.press2 + 0.2;
  const bars = barsSmooth(t, speech ? speechLevel : musicLevel);
  const lineOn = p.mode === "line";
  el.pillBarsBox.style.opacity = lineOn ? 0 : 1;
  el.pillBars.forEach((b, i) => { b.style.transform = `scaleY(${bars[i]})`; });
  el.pillLine.style.opacity = lineOn ? 1 : 0;
  el.pillLineFill.style.transform = `scaleX(${clamp(p.remaining)})`;
  // Recording: the mark breathes (--pill-pulse-duration 2s, 0.6 to 1).
  el.pillMark.style.opacity = 0.6 + 0.4 * (0.5 - 0.5 * Math.cos((2 * Math.PI * (t - T.pill)) / 2));
  // The appear flash: a hairline ring in accent that leaves the capsule.
  let flash = 0, fs = 1;
  for (const h of [T.pill, T.s5b]) if (t >= h && t < h + 0.6) { const u = (t - h) / 0.6; flash = (1 - u) * 0.9; fs = 1 + 0.7 * easeOutCubic(u); }
  css(el.pillflash, { opacity: flash, transform: `scale(${fs})` });
}

function scene3(t) {
  show(el.s3, t >= T.s3 - 0.05 && t < T.s4 + 0.02);
  if (t < T.s3 - 0.05 || t >= T.s4 + 0.02) return;
  const k = panel(t - (T.s3 + 0.05));
  css(el.mail, { opacity: clamp(k * 1.4), transform: `translateY(${(1 - k) * 90}px) scale(${lerp(0.93, 1, k)})` });
  // Steps: slide in, then light one at a time.
  const active = t < T.speakA ? 0 : t < T.press2 ? 1 : 2;
  [el.st1, el.st2, el.st3].forEach((s, i) => {
    const e = panel(t - (T.s3 + 0.12 + i * 0.08));
    const on = i === active;
    const litK = on ? 1 : 0.26;
    s.style.opacity = clamp(e * 1.3) * litK;
    s.style.transform = `translateX(${(1 - e) * -60}px) scale(${on ? 1 : 0.94})`;
    s.style.transformOrigin = "0 50%";
    s.querySelector(".n").style.color = on ? "var(--accent)" : "var(--ink-dark)";
  });
  // Live caption: the words rise out of the pill as they are spoken.
  const said = words.filter((w) => t >= w.a).map((w) => w.w).join(" ");
  el.captext.textContent = said;
  const cOut = easeStd(prog(t, T.press2, T.press2 + 0.12));
  css(el.caption, { left: `${1260 - 490}px`, top: `${930 - 26 - 260 - 6}px`, opacity: t < T.speakA - 0.05 ? 0 : 1 - cOut });
  // The paste lands: instant, with a highlight that fades.
  const pasted = t >= T.paste;
  el.mailwords.textContent = pasted ? CAPTION : "";
  el.mailhl.style.opacity = pasted ? 1 - easeOutCubic(prog(t, T.paste + 0.1, T.paste + 1.0)) : 0;
  el.mailhl.style.display = pasted ? "block" : "none";
  el.mailcaret.style.opacity = (pasted ? Math.floor((t - T.paste) / 0.5) % 2 === 1 : Math.floor(t / 0.5) % 2 === 0) ? 1 : 0;
  const pop = pasted ? 1 + 0.02 * Math.exp(-(t - T.paste) / 0.12) : 1;
  el.mailwords.parentElement.style.display = "inline";
  el.mailwords.parentElement.parentElement.style.transform = `scale(${pop})`;
  el.mailwords.parentElement.parentElement.style.transformOrigin = "0 0";
  // Keycap chip at each press.
  let kv = 0, kd = 0;
  for (const pt of [T.press1, T.press2]) {
    if (t >= pt - 0.2 && t < pt + 0.6) {
      kv = bouncy(t - (pt - 0.2)) * (1 - easeInCubic(prog(t, pt + 0.35, pt + 0.6)));
      kd = t >= pt ? Math.exp(-(t - pt) / 0.08) : 0;
    }
  }
  const px = t < T.s3 + 0.7 ? pillPose(t).x : 1260;
  const py = t < T.s3 + 0.7 ? pillPose(t).y : 930;
  css(el.kc1, { left: `${px + 130}px`, top: `${py - 30 + 5 * kd}px`, opacity: clamp(kv * 1.5), transform: `scale(${0.7 + 0.3 * kv})` });
}

function scene4(t) {
  show(el.s4, t >= T.s4 - 0.02 && t < T.zoom + 0.02);
  if (t < T.s4 - 0.02 || t >= T.zoom + 0.02) return;
  h4Chars.forEach((c, i) => {
    const e = panel(t - (T.s4 + 0.06 + i * 0.014));
    css(c, { opacity: clamp(e * 1.4), transform: `translateY(${(1 - e) * 44}px)` });
  });
  const arrived = T.cards.map((a) => panel(t - a));
  const shiftAll = arrived.slice(1).reduce((s, v) => s + v, 0);
  cardEls.forEach((c, i) => {
    const a = T.cards[i];
    const e = arrived[i];
    const depth = arrived.slice(i + 1).reduce((s, v) => s + v, 0);
    const x = lerp(1250, 0, e) - 150 * depth + 64 * shiftAll - 40;
    const y = lerp(80, 0, e) - 42 * depth;
    const z = lerp(260, 0, e) - 190 * depth;
    const ry = lerp(-46, -10, e) + 4 * depth;
    css(c.root, {
      opacity: t < a - 0.15 ? 0 : clamp((t - (a - 0.15)) / 0.12) * clamp(1 - 0.17 * depth),
      transform: `translate3d(${x}px, ${y}px, ${z}px) rotateY(${ry}deg) rotateX(4deg)`,
      filter: `brightness(${1 - 0.14 * depth})`,
      zIndex: `${10 + i}`,
    });
    const pasted = t >= a + 0.18;
    c.tx.textContent = pasted ? c.line : "";
    c.hl.style.opacity = pasted ? 1 - easeOutCubic(prog(t, a + 0.25, a + 0.9)) : 0;
    c.caret.style.opacity = Math.floor(t / 0.25) % 2 === 0 ? 1 : 0;
  });
  css(el.ticker, { transform: `translateX(${-120 * (t - T.s4) - 40}px)`, opacity: easeOutCubic(prog(t, T.s4 + 0.3, T.s4 + 0.7)) });
}

function scene5a(t) {
  show(el.s5a, t >= T.s5a && t < T.s5b);
  if (t < T.s5a || t >= T.s5b) return;
  const n = Math.round(99 * easeOutExpo(prog(t, T.s5a + 0.05, T.s5a + 0.75)));
  el.big99.textContent = String(n).padStart(2, "0");
  const out = easeInCubic(prog(t, T.s5b - 0.28, T.s5b - 0.02));
  const lk = panel(t - (T.s5a + 0.2));
  css(el.big99, { transform: `translateX(${-260 * out}px)`, opacity: 1 - out });
  css(el.langlabel, { opacity: clamp(lk * 1.4) * (1 - out), transform: `translate(${-260 * out}px, ${(1 - lk) * 30}px)` });
  const sk = panel(t - (T.s5a + 0.35));
  css(el.langsub, { opacity: clamp(sk * 1.4) * (1 - out), transform: `translate(${-260 * out}px, ${(1 - sk) * 30}px)` });
  const pos = rollPos(t);
  rollEls.forEach((r, i) => {
    const d = i - pos;
    css(r, { top: `${140 + d * 140}px`, opacity: Math.abs(d) > 2 ? 0 : (1 - Math.min(1, Math.abs(d)) * 0.6) * (1 - out), color: Math.abs(d) < 0.5 ? "var(--ink-dark)" : "var(--ink-dark-2)", transform: `translateX(${260 * out}px)` });
  });
}

function scene5b(t) {
  show(el.s5b, t >= T.s5b && t < T.s6 + 0.05);
  if (t < T.s5b || t >= T.s6 + 0.05) return;
  const k = panel(t - T.s5b);
  const dip = (pt) => (t >= pt ? Math.exp(-Math.max(0, t - pt - 0.04) / 0.09) * (t < pt + 0.04 ? (t - pt) / 0.04 : 1) : 0);
  const d = Math.max(dip(T.esc1), dip(T.esc2));
  css(el.esckey, {
    opacity: clamp(k * 1.5), top: `${lerp(560, 355, k) + 12 * d}px`,
    transform: `perspective(900px) rotateX(${lerp(40, 16, k)}deg) scale(${1 - 0.03 * d})`,
    boxShadow: `0 ${14 - 11 * d}px 0 #0b0a09, 0 30px 70px rgba(0,0,0,.55), 0 0 ${110 * d}px rgba(255,106,43,${0.5 * d}), inset 0 2px 0 rgba(255,244,230,.08)`,
  });
  const a = panel(t - (T.s5b + 0.2));
  const b = panel(t - (T.s5b + 0.38));
  css(el.escline1, { opacity: clamp(a * 1.4), transform: `translateY(${(1 - a) * 40}px)` });
  css(el.escline2, { opacity: clamp(b * 1.4), transform: `translateY(${(1 - b) * 30}px)` });
}

function scene6(t) {
  show(el.s6, t >= T.s6 && t < T.flood + 0.3);
  if (t < T.s6 || t >= T.flood + 0.3) return;
  chipEls.forEach((c, i) => {
    const e = bouncy(t - T.chips[i]);
    css(c, { opacity: t < T.chips[i] ? 0 : clamp(e * 2), transform: `translateY(${(1 - e) * -70}px) scale(${0.7 + 0.3 * e})` });
  });
  h6Chars.forEach((c, i) => {
    const e = panel(t - (T.h6 + i * 0.016));
    css(c, { opacity: clamp(e * 1.4), transform: `translateY(${(1 - e) * 50}px)` });
  });
}

function scene7(t) {
  show(el.s7, t >= T.flood);
  if (t < T.flood) return;
  const f = el.flood;
  if (t < T.flood + 0.25) {
    const r = 2300 * easeInCubic(prog(t, T.flood, T.flood + 0.25));
    css(f, { left: "0px", top: "0px", width: `${W}px`, height: `${H}px`, borderRadius: "0px", clipPath: `circle(${r}px at 960px 670px)` });
    el.ihead.style.opacity = 0;
    el.iquotes.style.opacity = 0;
    el["icon-shadow"].style.opacity = 0;
  } else {
    // The flood contracts into the app icon's squircle (824pt, r184 -> 280, r62.5).
    const u = easeInOutCubic(prog(t, T.flood + 0.25, T.land));
    const settle = t > T.land ? 1 + 0.07 * Math.exp(-(t - T.land) / 0.16) * Math.sin((t - T.land) * 26) : 1;
    const size = 280 * settle;
    const cx = 960, cy = 350;
    const w = lerp(W * 1.02, size, u), h = lerp(H * 1.02, size, u);
    const x = lerp(cx - W * 0.51, cx - size / 2, u), y = lerp(cy - H * 0.51 + 190, cy - size / 2, u);
    const rad = lerp(0, size * (184 / 824), u);
    css(f, { left: `${x}px`, top: `${y}px`, width: `${w}px`, height: `${h}px`, borderRadius: `${rad}px`, clipPath: "none" });
    css(el["icon-shadow"], { left: `${x}px`, top: `${y}px`, width: `${w}px`, height: `${h}px`, borderRadius: `${rad}px`, opacity: u });
    const hk = panel(t - (T.land - 0.05));
    el.ihead.style.opacity = t >= T.land - 0.05 ? 1 : 0;
    el.ihead.setAttribute("transform", `translate(0 ${(1 - hk) * 70})`);
    el.iquotes.style.opacity = 1;
    [[el.iq1, T.land + 0.16, 50.8, 50], [el.iq2, T.land + 0.24, 67.4, 48.8]].forEach(([q, at, qx, qy]) => {
      const e = bouncy(t - at);
      // The rail's glance (.mark-look-*): quotes slide, the head leans.
      q.setAttribute("transform", `translate(${qx} ${qy}) scale(${Math.max(0.0001, e)}) translate(${-qx} ${-qy})`);
    });
    const look = lookAt(t - 23.0);
    el.iquotes.setAttribute("transform", `translate(${look * 9 * 0.5} 0)`);
    el.ihead.firstElementChild.setAttribute("transform", `skewX(${look * 6 * 0.5})`);
  }
  const e1 = panel(t - (T.land + 0.3));
  const e2 = panel(t - (T.land + 0.5));
  const e3 = panel(t - (T.land + 0.7));
  css(el.wordmark, { opacity: clamp(e1 * 1.4), transform: `translateY(${(1 - e1) * 50}px)`, filter: `blur(${(1 - clamp(e1 * 1.2)) * 12}px)` });
  css(el.tagline, { opacity: clamp(e2 * 1.4), transform: `translateY(${(1 - e2) * 36}px)` });
  css(el.meta, { opacity: clamp(e3 * 1.4), transform: `translateY(${(1 - e3) * 30}px)` });
}
/* -1 = looking left, +1 = right; the rail's 9s glance compressed to 2.2s. */
function lookAt(u) {
  if (u <= 0 || u >= 2.2) return 0;
  const seg = [[0, 0], [0.25, -1], [0.75, -1], [1.0, 1], [1.5, 1], [1.8, 0], [2.2, 0]];
  for (let i = 0; i < seg.length - 1; i++) {
    const [a, va] = seg[i], [b, vb] = seg[i + 1];
    if (u >= a && u < b) return lerp(va, vb, easeStd((u - a) / (b - a)));
  }
  return 0;
}

/* Brand dot wipes: lattice dots swell to cover, then shrink to reveal. */
function drawWipe(t) {
  const ctx = el.wctx;
  ctx.clearRect(0, 0, W, H);
  const wipes = [
    { mid: T.wipe1, half: 0.25, color: "#ff6a2b" },
    { mid: T.wipe2, half: 0.25, color: "#f4f3ee" },
  ];
  for (const w of wipes) {
    if (t < w.mid - w.half || t > w.mid + w.half) continue;
    const pitch = 60, maxR = pitch * 0.74;
    ctx.fillStyle = w.color;
    const cover = t < w.mid;
    const p = cover ? prog(t, w.mid - w.half, w.mid) : prog(t, w.mid, w.mid + w.half);
    for (let y = pitch / 2; y < H + pitch; y += pitch) {
      for (let x = pitch / 2; x < W + pitch; x += pitch) {
        const d = (x / W) * 0.45 + (y / H) * 0.05;
        const q = easeInOutCubic(clamp(p * 1.5 - d));
        const r = maxR * (cover ? q : 1 - q);
        if (r < 0.3) continue;
        ctx.beginPath();
        ctx.arc(x, y, r, 0, Math.PI * 2);
        ctx.fill();
      }
    }
  }
}

function render(t) {
  camera(t);
  backgrounds(t);
  drawLattice(t);
  scene1(t);
  scene2(t);
  scene3(t);
  scene4(t);
  scene5a(t);
  scene5b(t);
  scene6(t);
  scene7(t);
  drawPill(t);
  drawFx(t);
  drawWipe(t);
}

window.TIMELINE = TIMELINE;
window.renderFrame = (t) => render(t);
window.__ready = (async () => {
  await document.fonts.load('800 100px "Doto"');
  await document.fonts.load('600 92px "Inter Display"');
  await document.fonts.load('900 330px "Inter Display"');
  await document.fonts.ready;
  build();
  render(0);
  return true;
})();
