// Snapora icon generator.
// Rasterizes the Snapora mark (viewfinder brackets + shutter dot on an ink
// rounded square) at 4x supersampling, writes PNGs for all required sizes and
// a multi-resolution .ico (PNG-compressed entries).
//
// Run: node scripts/gen-icons.mjs
// Writes into: src-tauri/icons/

import { deflateSync } from "node:zlib";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const OUT_DIR = join(dirname(fileURLToPath(import.meta.url)), "..", "src-tauri", "icons");
mkdirSync(OUT_DIR, { recursive: true });

// ---------------------------------------------------------------- PNG encoder
const CRC_TABLE = (() => {
  const t = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    t[n] = c >>> 0;
  }
  return t;
})();

function crc32(buf) {
  let c = 0xffffffff;
  for (let i = 0; i < buf.length; i++) c = CRC_TABLE[(c ^ buf[i]) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

function chunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length);
  const body = Buffer.concat([Buffer.from(type, "ascii"), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body));
  return Buffer.concat([len, body, crc]);
}

function encodePNG(width, height, rgba) {
  const sig = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0);
  ihdr.writeUInt32BE(height, 4);
  ihdr[8] = 8;  // bit depth
  ihdr[9] = 6;  // RGBA
  ihdr[10] = 0; ihdr[11] = 0; ihdr[12] = 0;
  const stride = width * 4;
  const raw = Buffer.alloc((stride + 1) * height);
  for (let y = 0; y < height; y++) {
    raw[y * (stride + 1)] = 0; // filter: none
    rgba.copy(raw, y * (stride + 1) + 1, y * stride, (y + 1) * stride);
  }
  return Buffer.concat([
    sig,
    chunk("IHDR", ihdr),
    chunk("IDAT", deflateSync(raw, { level: 9 })),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}

// --------------------------------------------------------------- raster layer
// Colors (linear-ish, straight RGBA)
const INK = [18, 22, 30, 255];        // #12161E deep graphite-ink square
const PAPER = [244, 245, 247, 255];   // bracket color (near-white)
const AMBER = [238, 141, 37, 255];    // #EE8D25 safelight shutter dot

function hex(h) {
  return [parseInt(h.slice(1, 3), 16), parseInt(h.slice(3, 5), 16), parseInt(h.slice(5, 7), 16), 255];
}

// Draw into an RGBA buffer of size W. px(x, y, rgba) paints with blending.
function makeCanvas(W) {
  const buf = Buffer.alloc(W * W * 4);
  const blend = (i, c, a) => {
    const ea = a / 255;
    const old = buf[i + 3] / 255;
    const outA = ea + old * (1 - ea);
    if (outA <= 0) return;
    for (let k = 0; k < 3; k++) {
      buf[i + k] = Math.round((c[k] * ea + buf[i + k] * old * (1 - ea)) / outA);
    }
    buf[i + 3] = Math.round(outA * 255);
  };
  return {
    W,
    px(x, y, c, a = 255) {
      x |= 0; y |= 0;
      if (x < 0 || y < 0 || x >= W || y >= W) return;
      const i = (y * W + x) * 4;
      blend(i, c, a);
    },
    buf,
  };
}

// distance field helpers in unit square [0,1]
const distRect = (px, py, cx, cy, hw, hh, r) => {
  // signed distance to rounded rect (approx, r <= min(hw,hh))
  const dx = Math.abs(px - cx) - (hw - r);
  const dy = Math.abs(py - cy) - (hh - r);
  const ox = Math.max(dx, 0), oy = Math.max(dy, 0);
  return Math.hypot(ox, oy) + Math.min(Math.max(dx, dy), 0) - r;
};

const distCircle = (px, py, cx, cy, r) => Math.hypot(px - cx, py - cy) - r;

// corner bracket: union of two thin rounded rects at a corner
function bracketDist(px, py, ox, oy, sx, sy, L, t, r) {
  // arm along x: center at (ox + sx*L/2, oy + sy*t/2)
  const d1 = distRect(px, py, ox + sx * (L / 2), oy + sy * (t / 2), L / 2, t / 2, r);
  const d2 = distRect(px, py, ox + sx * (t / 2), oy + sy * (L / 2), t / 2, L / 2, r);
  return Math.min(d1, d2);
}

function renderAt(S) {
  const cv = makeCanvas(S);
  const u = (v) => v * S; // unit → pixels
  // background rounded square
  const pad = 0.055, rad = 0.215;
  for (let y = 0; y < S; y++) {
    for (let x = 0; x < S; x++) {
      const px = (x + 0.5) / S, py = (y + 0.5) / S;
      const d = distRect(px, py, 0.5, 0.5, 0.5 - pad, 0.5 - pad, rad);
      const cov = Math.min(1, Math.max(0, 0.5 - d * S)); // AA over half pixel
      if (cov > 0) cv.px(x, y, INK, Math.round(cov * 255));
    }
  }
  // viewfinder brackets
  const bl = 0.205, bt = 0.055, br = 0.02;
  const corners = [
    [0.205, 0.205, 1, 1],
    [0.795, 0.205, -1, 1],
    [0.205, 0.795, 1, -1],
    [0.795, 0.795, -1, -1],
  ];
  for (let y = 0; y < S; y++) {
    for (let x = 0; x < S; x++) {
      const px = (x + 0.5) / S, py = (y + 0.5) / S;
      let d = Infinity;
      for (const [ox, oy, sx, sy] of corners) {
        d = Math.min(d, bracketDist(px, py, ox, oy, sx, sy, bl, bt, br));
      }
      const cov = Math.min(1, Math.max(0, 0.5 - d * S));
      if (cov > 0) cv.px(x, y, PAPER, Math.round(cov * 255));
    }
  }
  // shutter dot
  const dotR = 0.115;
  for (let y = 0; y < S; y++) {
    for (let x = 0; x < S; x++) {
      const px = (x + 0.5) / S, py = (y + 0.5) / S;
      const d = distCircle(px, py, 0.5, 0.5, dotR);
      const cov = Math.min(1, Math.max(0, 0.5 - d * S));
      if (cov > 0) cv.px(x, y, AMBER, Math.round(cov * 255));
    }
  }
  return cv.buf;
}

function boxDownsample(buf, from, to) {
  const out = Buffer.alloc(to * to * 4);
  const f = from / to;
  for (let y = 0; y < to; y++) {
    for (let x = 0; x < to; x++) {
      let r = 0, g = 0, b = 0, a = 0, n = 0;
      const x0 = Math.floor(x * f), x1 = Math.max(x0 + 1, Math.floor((x + 1) * f));
      const y0 = Math.floor(y * f), y1 = Math.max(y0 + 1, Math.floor((y + 1) * f));
      for (let yy = y0; yy < y1; yy++) {
        for (let xx = x0; xx < x1; xx++) {
          const i = (yy * from + xx) * 4;
          r += buf[i]; g += buf[i + 1]; b += buf[i + 2]; a += buf[i + 3]; n++;
        }
      }
      const o = (y * to + x) * 4;
      out[o] = Math.round(r / n); out[o + 1] = Math.round(g / n);
      out[o + 2] = Math.round(b / n); out[o + 3] = Math.round(a / n);
    }
  }
  return out;
}

// ------------------------------------------------------------------ ICO writer
function makeIco(pngBuffers) {
  // entries: [{size, data}]
  const count = pngBuffers.length;
  const header = Buffer.alloc(6);
  header.writeUInt16LE(0, 0); header.writeUInt16LE(1, 2); header.writeUInt16LE(count, 4);
  const dirSize = 16 * count;
  let offset = 6 + dirSize;
  const dirs = [];
  for (const { size, data } of pngBuffers) {
    const d = Buffer.alloc(16);
    d[0] = size >= 256 ? 0 : size; // width
    d[1] = size >= 256 ? 0 : size; // height
    d[2] = 0; d[3] = 0;
    d.writeUInt16LE(1, 4);  // color plane
    d.writeUInt16LE(32, 6); // bits per pixel
    d.writeUInt32LE(data.length, 8);
    d.writeUInt32LE(offset, 12);
    offset += data.length;
    dirs.push(d);
  }
  return Buffer.concat([header, ...dirs, ...pngBuffers.map((p) => p.data)]);
}

// --------------------------------------------------------------------- outputs
const SS = 1024; // 4x supersample of 256
console.log("Rendering at", SS, "px supersample…");
const hi = renderAt(SS);

const sizes = [16, 24, 32, 48, 64, 128, 256, 512];
const pngs = {};
for (const s of sizes) {
  const buf = boxDownsample(hi, SS, s);
  pngs[s] = encodePNG(s, s, buf);
}

const files = [
  ["32x32.png", 32],
  ["128x128.png", 128],
  ["128x128@2x.png", 256],
  ["icon.png", 512],
  ["Square30x30Logo.png", 30],
  ["Square44x44Logo.png", 44],
  ["Square71x71Logo.png", 71],
  ["Square89x89Logo.png", 89],
  ["Square107x107Logo.png", 107],
  ["Square142x142Logo.png", 142],
  ["Square150x150Logo.png", 150],
  ["Square284x284Logo.png", 284],
  ["Square310x310Logo.png", 310],
  ["StoreLogo.png", 50],
];
for (const [name, s] of files) {
  writeFileSync(join(OUT_DIR, name), pngs[s] ?? encodePNG(s, s, boxDownsample(hi, SS, s)));
}

// icon.ico: 16..256
const icoSizes = [16, 24, 32, 48, 64, 128, 256];
const ico = makeIco(icoSizes.map((s) => ({ size: s, data: pngs[s] })));
writeFileSync(join(OUT_DIR, "icon.ico"), ico);

console.log("Wrote icons to", OUT_DIR);
