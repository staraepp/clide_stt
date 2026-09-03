// Generates the Clide source icon (1024x1024 PNG) with no image dependencies.
// `npx tauri icon` derives every platform size from this file.
import { deflateSync } from "node:zlib";
import { writeFileSync, mkdirSync } from "node:fs";

const S = 1024;
const px = new Uint8Array(S * S * 4);

const clamp01 = (v) => (v < 0 ? 0 : v > 1 ? 1 : v);
const smooth = (edge, w, d) => clamp01(0.5 - (d - edge) / w);
const mix = (a, b, t) => a + (b - a) * t;

// Signed distance to a rounded rectangle centred on the canvas.
function sdRoundRect(x, y, halfW, halfH, r) {
  const qx = Math.abs(x) - halfW + r;
  const qy = Math.abs(y) - halfH + r;
  const ax = Math.max(qx, 0);
  const ay = Math.max(qy, 0);
  return Math.min(Math.max(qx, qy), 0) + Math.hypot(ax, ay) - r;
}

// Five bars, tallest in the middle: the same waveform the HUD draws.
const bars = [0.3, 0.62, 1.0, 0.5, 0.36];
const barW = 46;
const gap = 42;
const maxH = 300;
const totalW = bars.length * barW + (bars.length - 1) * gap;

for (let py = 0; py < S; py++) {
  for (let pxi = 0; pxi < S; pxi++) {
    const x = pxi - S / 2 + 0.5;
    const y = py - S / 2 + 0.5;

    // Base: deep blue-black, lifted toward the top-left.
    const g = clamp01(0.5 - (x + y) / (S * 1.6));
    let r = mix(4, 12, g);
    let gg = mix(9, 28, g);
    let b = mix(15, 44, g);

    // Ambient icy glow behind the mark.
    const glow = Math.exp(-(x * x + y * y) / (2 * 210 * 210)) * 0.5;
    r += 20 * glow;
    gg += 90 * glow;
    b += 120 * glow;

    // Waveform bars.
    let ink = 0;
    for (let i = 0; i < bars.length; i++) {
      const cx = -totalW / 2 + barW / 2 + i * (barW + gap);
      const h = (maxH * bars[i]) / 2;
      const d = sdRoundRect(x - cx, y, barW / 2, h, barW / 2);
      ink = Math.max(ink, smooth(0, 2.0, d));
      // Tight bloom around each bar so the mark reads as emissive.
      ink = Math.max(ink, smooth(0, 34, d) * 0.22);
    }

    // Icy blue -> near-white core, brighter toward the centre bars.
    const heat = clamp01(1 - Math.abs(x) / (totalW * 0.6));
    const ir = mix(92, 214, heat);
    const ig = mix(187, 236, heat);
    const ib = mix(228, 252, heat);
    r = mix(r, ir, ink);
    gg = mix(gg, ig, ink);
    b = mix(b, ib, ink);

    // Squircle mask matching macOS icon geometry.
    const mask = smooth(0, 2.0, sdRoundRect(x, y, 452, 452, 176));

    const o = (py * S + pxi) * 4;
    px[o] = Math.round(clamp01(r / 255) * 255);
    px[o + 1] = Math.round(clamp01(gg / 255) * 255);
    px[o + 2] = Math.round(clamp01(b / 255) * 255);
    px[o + 3] = Math.round(mask * 255);
  }
}

// --- minimal PNG encoder ---
const raw = Buffer.alloc(S * (S * 4 + 1));
for (let y = 0; y < S; y++) {
  raw[y * (S * 4 + 1)] = 0; // filter: none
  Buffer.from(px.buffer, y * S * 4, S * 4).copy(raw, y * (S * 4 + 1) + 1);
}

const crcTable = Array.from({ length: 256 }, (_, n) => {
  let c = n;
  for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
  return c >>> 0;
});
const crc32 = (buf) => {
  let c = 0xffffffff;
  for (const byte of buf) c = crcTable[(c ^ byte) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
};
const chunk = (type, data) => {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length);
  const body = Buffer.concat([Buffer.from(type, "ascii"), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body));
  return Buffer.concat([len, body, crc]);
};
const ihdr = Buffer.alloc(13);
ihdr.writeUInt32BE(S, 0);
ihdr.writeUInt32BE(S, 4);
ihdr[8] = 8; // bit depth
ihdr[9] = 6; // RGBA
mkdirSync("src-tauri/icons", { recursive: true });
writeFileSync(
  "src-tauri/icons/source.png",
  Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk("IHDR", ihdr),
    chunk("IDAT", deflateSync(raw, { level: 9 })),
    chunk("IEND", Buffer.alloc(0)),
  ]),
);
console.log("wrote src-tauri/icons/source.png");
