// @pxl/web/modules/world/billboard/canvas_text.ts

/**
 * Pure canvas-text helpers shared by every world-anchored billboard (tier 3
 * of discussions/20260907-world-rendering-tiers.md) that draws its own
 * content onto a 2D canvas rather than sourcing an image asset — chat_bubble.tsx
 * today, a future POI label tomorrow. No `three` import: this file only ever
 * touches CanvasRenderingContext2D, so it stays usable from anything that
 * needs wrapped text on a canvas, not just a THREE.Sprite consumer.
 */

/** Greedy word wrap against ctx's currently-set font, then a per-character
 * fallback pass for any line still too wide on its own — a single long
 * word, or (the common case that broke this the first time around) a
 * Korean message typed with no spacing between words at all, which the
 * word pass alone treats as one unbreakable "word" spanning the whole
 * message. */
export function wrap_text(ctx: CanvasRenderingContext2D, text: string, max_width: number): string[] {
  const words = text.split(/\s+/).filter((w) => w.length > 0);
  if (words.length === 0) return [""];

  const word_lines: string[] = [];
  let current = words[0];
  for (let i = 1; i < words.length; i++) {
    const candidate = `${current} ${words[i]}`;
    if (ctx.measureText(candidate).width <= max_width) {
      current = candidate;
    } else {
      word_lines.push(current);
      current = words[i];
    }
  }
  word_lines.push(current);

  const lines: string[] = [];
  for (const line of word_lines) {
    if (ctx.measureText(line).width <= max_width) {
      lines.push(line);
      continue;
    }
    let chunk = "";
    for (const ch of line) {
      const candidate = chunk + ch;
      if (chunk !== "" && ctx.measureText(candidate).width > max_width) {
        lines.push(chunk);
        chunk = ch;
      } else {
        chunk = candidate;
      }
    }
    if (chunk !== "") lines.push(chunk);
  }
  return lines;
}
