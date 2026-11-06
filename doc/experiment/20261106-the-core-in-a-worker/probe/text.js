// The text the kernel calls for, measured and drawn as services/text.ts does,
// on whichever side imports it.

export const SAMPLES = [];
for (const text of ["Pixelet 12:30", "안녕하세요 세계", "日本語のテキスト、です。", "漢字 中文", "😀👍🏽 emoji", "ÄÖÜ ßç ñ"])
  for (const weight of [400, 700])
    for (const px of [12, 16, 24]) SAMPLES.push({ text, weight, px });

function hash(bytes) {
  let h = 2166136261;
  for (let i = 0; i < bytes.length; i++) { h ^= bytes[i]; h = Math.imul(h, 16777619); }
  return h >>> 0;
}

export async function measure() {
  if (typeof OffscreenCanvas === "undefined") return null;
  const ctx = new OffscreenCanvas(1, 1).getContext("2d", { willReadFrequently: true });
  if (!ctx) return null;
  return SAMPLES.map(({ text, weight, px }) => {
    ctx.font = `${weight} ${px}px system-ui`;
    const width = ctx.measureText(text).width;
    const w = Math.ceil(width) + 4, h = Math.ceil(px * 1.5);
    ctx.canvas.width = w; ctx.canvas.height = h;
    ctx.clearRect(0, 0, w, h);
    ctx.font = `${weight} ${px}px system-ui`;
    ctx.fillStyle = "rgba(20,30,40,1)";
    ctx.textAlign = "left";
    ctx.textBaseline = "middle";
    ctx.fillText(text, 2, h / 2);
    return { width, hash: hash(ctx.getImageData(0, 0, w, h).data) };
  });
}
