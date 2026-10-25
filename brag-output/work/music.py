"""Original score + SFX for the Vocs film, synthesized from scratch.

120 BPM (1 beat = 0.5s = 15 frames), F minor, every hit on the grid the edit
uses. Cue times come from timeline.json, which the composition exports, so the
sound and the picture share one clock.

    python3 -I music.py <timeline.json> <out.wav>
"""
import json
import sys
import wave

import numpy as np

SR = 48000
DUR = 25.0
N = int(SR * DUR)
BEAT = 0.5
rng = np.random.default_rng(5)

tl = json.load(open(sys.argv[1]))
OUT = sys.argv[2]


def mtof(m):
    return 440.0 * 2 ** ((m - 69) / 12)


def buf():
    return np.zeros((N, 2))


def idx(t):
    return int(round(t * SR))


def add(bus, sig, t, gain=1.0, pan=0.0):
    """Mix a mono or stereo signal into a stereo bus at time t, constant-power pan."""
    i = idx(t)
    if i >= N:
        return
    if sig.ndim == 1:
        l, r = np.cos((pan + 1) * np.pi / 4), np.sin((pan + 1) * np.pi / 4)
        sig = np.stack([sig * l, sig * r], axis=1) * np.sqrt(2)
    j = min(N, i + len(sig))
    s0 = max(0, -i)
    bus[max(0, i):j] += sig[s0 : s0 + (j - max(0, i))] * gain


def fft_band(sig, lo=None, hi=None, slope=1.0):
    """Zero-phase band filter with soft (Butterworth-like) edges."""
    n = len(sig)
    spec = np.fft.rfft(sig, axis=0)
    f = np.fft.rfftfreq(n, 1 / SR)
    g = np.ones_like(f)
    if lo:
        g *= 1 / np.sqrt(1 + (lo / np.maximum(f, 1e-3)) ** (4 * slope))
    if hi:
        g *= 1 / np.sqrt(1 + (f / hi) ** (4 * slope))
    if sig.ndim == 2:
        g = g[:, None]
    return np.fft.irfft(spec * g, n=n, axis=0)


def env_adsr(n, a, d, s, r_start, r):
    t = np.arange(n) / SR
    e = np.where(t < a, t / max(a, 1e-4), s + (1 - s) * np.exp(-(t - a) / max(d, 1e-4)))
    e = np.where(t > r_start, e * np.exp(-(t - r_start) / max(r, 1e-4)), e)
    return e


# Band-limited saw wavetable (harmonics kept under ~12kHz for the pad range).
TABLE = 4096
_ph = np.arange(TABLE) / TABLE
SAW = sum(np.sin(2 * np.pi * k * _ph) / k for k in range(1, 28)) * (2 / np.pi)


def saw(freq, n, phase0=0.0):
    ph = (phase0 + np.cumsum(np.full(n, freq / SR))) % 1.0
    return np.interp(ph * TABLE, np.arange(TABLE), SAW)


# ── instruments ────────────────────────────────────────────────────────────
def kick(gain=1.0):
    n = int(0.5 * SR)
    t = np.arange(n) / SR
    f = 46 + 120 * np.exp(-t / 0.032)
    body = np.sin(2 * np.pi * np.cumsum(f) / SR) * np.exp(-t / 0.19)
    click = fft_band(rng.standard_normal(n) * np.exp(-t / 0.0025), lo=1500) * 0.35
    return np.tanh((body + click) * 1.6) * gain


def clap():
    n = int(0.4 * SR)
    t = np.arange(n) / SR
    e = np.zeros(n)
    for k, off in enumerate((0.0, 0.009, 0.018)):
        tt = np.clip(t - off, 0, None)
        e += (t >= off) * np.exp(-tt / 0.006) * (0.8 if k < 2 else 1.0)
    e += (t >= 0.018) * 0.55 * np.exp(-np.clip(t - 0.018, 0, None) / 0.11)
    return fft_band(rng.standard_normal(n) * e, lo=900, hi=5200)


def hat(open_=False):
    n = int((0.35 if open_ else 0.08) * SR)
    t = np.arange(n) / SR
    return fft_band(rng.standard_normal(n), lo=7000) * np.exp(-t / (0.12 if open_ else 0.022))


def bass_note(m, dur):
    n = int(dur * SR)
    t = np.arange(n) / SR
    f = mtof(m)
    s = np.sin(2 * np.pi * f * t) + 0.35 * np.sin(4 * np.pi * f * t) + 0.12 * saw(f, n)
    e = env_adsr(n, 0.004, 0.12, 0.65, dur - 0.04, 0.02)
    return np.tanh(s * 1.4) * e


def pad_chord(notes, dur, bright=1.0):
    n = int(dur * SR)
    out = np.zeros((n, 2))
    for m in notes:
        for d, pan in ((-0.09, -0.6), (0.0, 0.0), (0.1, 0.6)):
            v = saw(mtof(m) * 2 ** (d / 12), n, rng.random())
            l, r = np.cos((pan + 1) * np.pi / 4), np.sin((pan + 1) * np.pi / 4)
            out[:, 0] += v * l
            out[:, 1] += v * r
    out = fft_band(out, hi=1100 * bright, slope=0.8)
    e = env_adsr(n, 0.08, 0.6, 0.8, dur - 0.12, 0.08)
    return out * e[:, None] / (len(notes) * 2.2)


def pluck(m, dur=0.5):
    n = int(dur * SR)
    t = np.arange(n) / SR
    f = mtof(m)
    s = sum(np.sin(2 * np.pi * f * k * t) * np.exp(-t * (7 + 5 * k)) / k for k in range(1, 7))
    return s * np.minimum(1, t / 0.002)


def bell(m, dur=1.6):
    n = int(dur * SR)
    t = np.arange(n) / SR
    f = mtof(m)
    s = sum(a * np.sin(2 * np.pi * f * r * t) * np.exp(-t / dk) for r, a, dk in ((1, 1, 0.9), (2.0, 0.45, 0.5), (2.76, 0.3, 0.3), (5.4, 0.15, 0.12)))
    return s * np.minimum(1, t / 0.002)


def blip(m, dur=0.06):
    n = int(dur * SR)
    t = np.arange(n) / SR
    return np.sin(2 * np.pi * mtof(m) * t) * np.exp(-t / 0.018) * np.minimum(1, t / 0.001)


def keytap(seed):
    r = np.random.default_rng(seed)
    n = int(0.06 * SR)
    t = np.arange(n) / SR
    tick = fft_band(r.standard_normal(n), lo=1800 + r.random() * 900, hi=9000) * np.exp(-t / 0.006)
    thock = np.sin(2 * np.pi * (180 + r.random() * 60) * t) * np.exp(-t / 0.012) * 0.5
    return tick + thock


def thock(big=True):
    n = int((0.9 if big else 0.25) * SR)
    t = np.arange(n) / SR
    body = np.sin(2 * np.pi * np.cumsum(120 * np.exp(-t / 0.05) + 55) / SR) * np.exp(-t / (0.12 if big else 0.05))
    click = fft_band(rng.standard_normal(n), lo=600, hi=4000) * np.exp(-t / 0.01) * 0.6
    sub = np.sin(2 * np.pi * mtof(29) * t) * np.exp(-t / 0.5) * (0.8 if big else 0)
    return np.tanh((body + click + sub) * 1.3)


def whoosh(dur=0.5, rise=0.6):
    n = int(dur * SR)
    t = np.arange(n) / SR
    noise = rng.standard_normal(n)
    lo, mid, hi = fft_band(noise, 200, 900), fft_band(noise, 900, 3000), fft_band(noise, 3000, 9000)
    p = t / dur
    sweep = np.clip(p / rise, 0, 1)
    s = lo * (1 - sweep) + mid * np.sin(np.pi * sweep) + hi * sweep * 0.6
    e = np.sin(np.pi * np.clip(p, 0, 1)) ** 1.6
    out = np.zeros((n, 2))
    pan = -0.8 + 1.6 * p
    out[:, 0] = s * e * np.cos((pan + 1) * np.pi / 4)
    out[:, 1] = s * e * np.sin((pan + 1) * np.pi / 4)
    return out


def riser(dur):
    n = int(dur * SR)
    t = np.arange(n) / SR
    p = t / dur
    noise = fft_band(rng.standard_normal(n), lo=500)
    tone = np.sin(2 * np.pi * np.cumsum(mtof(53) * 2 ** (p * 2)) / SR) * 0.25
    return (noise * 0.6 + tone) * p ** 2.2


def crash(dur=2.4):
    n = int(dur * SR)
    t = np.arange(n) / SR
    return fft_band(rng.standard_normal(n), lo=3500) * np.exp(-t / 0.7)


# ── arrangement ────────────────────────────────────────────────────────────
F, Ab, C, Db, Eb, G, Bb = 53, 56, 60, 49, 51, 55, 58  # F3 region
CHORDS = {"Fm": [F, Ab, C], "Db": [Db + 12, F, Ab], "Ab": [Ab, C, Eb + 12], "Eb": [Eb + 12, G, Bb]}
ROOTS = {"Fm": 41, "Db": 37, "Ab": 44, "Eb": 39}
BARS = [(3.0, "Fm"), (5.0, "Db"), (7.0, "Ab"), (9.0, "Eb"), (11.0, "Fm"), (13.0, "Db"), (15.0, "Ab"), (17.0, "Eb"), (19.0, "Fm"), (21.0, "Db")]

drums, bass, pads, plucks, sfx = buf(), buf(), buf(), buf(), buf()

# Intro: a dark Fm bed under the typing, swelling into the press.
add(pads, pad_chord(CHORDS["Fm"], 3.0, bright=0.45), 0.0, 0.8)
for k, t in enumerate(tl["keys"]):
    add(sfx, keytap(k), t, 0.33, pan=(k % 5 - 2) * 0.12)
add(sfx, riser(0.9), 1.6, 0.18)

# The press, the suck-in, the drop.
add(sfx, thock(True), 2.5, 0.75)
add(sfx, fft_band(riser(0.5), hi=6000), 2.5, 0.22)
add(sfx, crash(1.6), 3.0, 0.16)

# Groove 3.0-21.5, a breath before the land at 22.0.
kicks = [3.0 + i * BEAT for i in range(int((21.5 - 3.0) / BEAT))]
for t in kicks:
    add(drums, kick(), t, 0.72)
for t in kicks:
    add(drums, hat(open_=(11.0 <= t < 21.0 and int((t - 3.0) / BEAT) % 4 == 3)), t + BEAT / 2, 0.16 if t >= 5.0 else 0.08, pan=0.25)
for i in range(int((21.5 - 5.0) / (BEAT / 2))):
    t = 5.0 + i * BEAT / 2
    if i % 2 == 1:
        add(drums, hat(), t, 0.05, pan=-0.3)
for t in kicks:
    if t >= 5.0 and int((t - 3.0) / BEAT) % 2 == 1:
        add(drums, clap(), t, 0.32 if t >= 11.0 else 0.22)
# Fill into the land: a 16th clap roll rising.
for i in range(8):
    add(drums, clap(), 21.0 + i * BEAT / 4, 0.08 + i * 0.03)

for start, ch in BARS:
    dur = 2.0 if start < 21.0 else 1.0
    add(pads, pad_chord(CHORDS[ch], dur, bright=1.0 if start >= 11.0 else 0.75), start, 1.0)
    for e in range(int(dur / (BEAT / 2))):
        t = start + e * BEAT / 2
        if t >= 21.5:
            break
        add(bass, bass_note(ROOTS[ch] + (12 if e % 4 == 3 else 0), BEAT / 2 - 0.01), t, 0.42)
    if start >= 5.0:
        arp = CHORDS[ch] + [CHORDS[ch][0] + 12]
        for e in range(int(dur / (BEAT / 4))):
            t = start + e * BEAT / 4
            if 21.5 <= t:
                break
            add(plucks, pluck(arp[[0, 2, 1, 3, 2, 1, 3, 0][e % 8]] + 12), t, 0.10 if e % 4 else 0.14, pan=0.35 if e % 2 else -0.35)

# Sidechain: everything melodic ducks under the kick.
duck = np.ones(N)
for t in kicks:
    i = idx(t)
    seg = np.arange(min(N - i, int(0.35 * SR))) / SR
    duck[i : i + len(seg)] = np.minimum(duck[i : i + len(seg)], 1 - 0.55 * np.exp(-seg / 0.09))
for b in (bass, pads, plucks):
    b *= duck[:, None]

# Land at 22.0: the stab, the crash, the sub; then a quiet tag to the end.
add(drums, kick(1.2), 22.0, 1.0)
add(sfx, crash(3.0), 22.0, 0.22)
add(bass, bass_note(29, 2.6), 22.0, 0.5)
add(pads, pad_chord([41 + 12, F, Ab, C, F + 12], 3.0, bright=1.6), 22.0, 1.25)
for k, m in enumerate([F + 24, C + 12, Ab + 12, F + 12]):
    add(plucks, bell(m, 1.4), 22.6 + k * BEAT / 2, 0.16)

# Delay on plucks (dotted eighth, ping-pong) for space.
d = idx(0.375)
echo = np.zeros_like(plucks)
echo[d:, 0] += plucks[:-d, 1] * 0.38
echo[d:, 1] += plucks[:-d, 0] * 0.38
echo[2 * d :, :] += plucks[: -2 * d, :] * 0.16
plucks += fft_band(echo, hi=4000)

# ── SFX from the edit ──────────────────────────────────────────────────────
for t in tl["whoosh"]:
    add(sfx, whoosh(0.5), t - 0.1, 0.2)
add(sfx, riser(0.6), 13.4, 0.16)
add(sfx, riser(0.55), 20.95, 0.2)
for t in tl["presses"]:
    add(sfx, thock(False), t, 0.35)
add(sfx, thock(True), 16.0, 0.35)
add(sfx, crash(1.2), 16.0, 0.08)
for c in tl["chimes"]:
    add(sfx, bell(77, 1.8), c["t"], 0.2)
    add(sfx, bell(84, 1.4), c["t"] + 0.06, 0.1)
penta = [65, 68, 70, 72, 75, 77, 80, 82, 84, 87, 89, 92]
roll = [x for x in tl["ticks"] if x["kind"] == "roll"]
for k, x in enumerate(roll):
    add(sfx, blip(penta[k % len(penta)]), x["t"], 0.1, pan=0.4)
for k, x in enumerate([x for x in tl["ticks"] if x["kind"] == "chip"]):
    add(sfx, blip([72, 75, 77, 80][k], 0.12), x["t"], 0.2)
    add(sfx, keytap(100 + k), x["t"], 0.06)
for k, x in enumerate([x for x in tl["ticks"] if x["kind"] == "paste"]):
    add(sfx, bell([77, 80, 84, 89][k], 0.6), x["t"], 0.07, pan=(k - 1.5) * 0.3)

# ── reverb (convolution with a synthetic hall) ─────────────────────────────
def reverb(sig, rt=0.65, pre=0.018):
    n = int(2.2 * SR)
    t = np.arange(n) / SR
    ir = rng.standard_normal((n, 2)) * np.exp(-t / rt)[:, None]
    ir = fft_band(ir, lo=180, hi=7000)
    ir = np.concatenate([np.zeros((int(pre * SR), 2)), ir])
    size = 1 << int(np.ceil(np.log2(len(sig) + len(ir))))
    out = np.fft.irfft(np.fft.rfft(sig, size, axis=0) * np.fft.rfft(ir, size, axis=0), size, axis=0)[: len(sig)]
    return out / np.max(np.abs(ir).sum(axis=0)) * 9


music = drums + bass + pads * 1.35 + plucks * 1.5
send = pads * 0.45 + plucks * 0.5 + drums * 0.08 + sfx * 0.2

# Escape: the music bus closes as the countdown drains and springs open on refill.
for f in tl["filter"]:
    a, b, c = idx(f["a"]), idx(f["b"]), idx(f["c"] + 0.15)
    cut = np.full(c - a, 16000.0)
    cut[: b - a] = 16000 * (420 / 16000) ** (np.arange(b - a) / (b - a))
    k = np.arange(c - b) / (c - b)
    cut[b - a :] = 420 * (16000 / 420) ** (1 - (1 - k) ** 3)
    alpha = 1 - np.exp(-2 * np.pi * cut / SR)
    for ch in range(2):
        for stage in range(2):
            x = music[a:c, ch].copy()
            y = np.empty_like(x)
            z = x[0]
            for i in range(len(x)):
                z += alpha[i] * (x[i] - z)
                y[i] = z
            music[a:c, ch] = y

mix = music + sfx + reverb(send)
mix = fft_band(mix, lo=28)
# Glue: gentle soft clip, then leave headroom for loudness normalisation.
for name, b in (("drums", drums), ("bass", bass), ("pads", pads), ("plucks", plucks), ("sfx", sfx)):
    print(f"{name:7s} rms {np.sqrt(np.mean(b**2)):.3f} peak {np.max(np.abs(b)):.2f}")
print("pre-clip peak", np.max(np.abs(mix)), "rms", np.sqrt(np.mean(mix**2)))
mix = mix / np.max(np.abs(mix)) * 1.4
mix = np.tanh(mix)
fade = np.ones(N)
fade[-idx(0.5) :] = np.linspace(1, 0, idx(0.5)) ** 2
mix *= fade[:, None]
mix /= np.max(np.abs(mix)) / 0.89

with wave.open(OUT, "wb") as w:
    w.setnchannels(2)
    w.setsampwidth(2)
    w.setframerate(SR)
    w.writeframes((mix * 32767).astype("<i2").tobytes())
print("wrote", OUT, f"{N / SR:.2f}s")
