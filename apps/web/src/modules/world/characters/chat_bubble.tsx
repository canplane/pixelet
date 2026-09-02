// @pxl/web/modules/world/characters/chat_bubble.tsx

/**
 * A billboarded speech bubble over the companion's head, showing the most
 * recent line from the (currently dev-only) map chat — see
 * modules/map/ui/overlay/mode_bar.tsx / dummy_chat_text.tsx for where
 * chatLines actually comes from. Pure THREE.Sprite (not a DOM overlay):
 * always faces the camera regardless of the character's own rotation.z,
 * with no lookAt bookkeeping needed — that's what a Sprite's own material
 * does by construction.
 *
 * The bubble itself is drawn on a plain 2D canvas (black notched-corner
 * border, white fill, a downward tail) rather than sourced from any image
 * asset — its size has to track the message's own text, which a fixed-
 * image asset would still need runtime 9-slice code to stretch, so
 * drawing it procedurally costs the same and skips an external CC0
 * dependency for something this simple. THREE.NearestFilter on the
 * resulting texture is what actually gives the "pixel" read: the canvas
 * itself is drawn with regular anti-aliased text (readability), and
 * Nearest sampling keeps the border/corner edges hard instead of blurring
 * them when the sprite is scaled in the 3D scene — same idea as the
 * terrain/character textures elsewhere in this pipeline, just applied to
 * a canvas-drawn texture instead of a decoded PNG.
 */

import { useEffect, useMemo, useState } from "react";
import * as THREE from "three";

import type { f64 } from "@xpute/core/kit/word.ts";
import { useUiStore } from "@/app/store/ui_store.ts";

// A new chat line shows for this long, then the bubble disappears — chat
// itself has no such expiry (dummy_chat_text.tsx's log keeps up to 10
// lines forever), so this is tracked locally here, not in ui_store.
const SHOW_DURATION_MS = 8000;

const FONT = "bold 16px ui-monospace, SFMono-Regular, Menlo, monospace";
const PADDING_X = 14;
const PADDING_Y = 10;
const LINE_H = 19;
const BORDER = 3;
const NOTCH = 4;
const TAIL_W = 14;
const TAIL_H = 10;

// A long message wraps onto more lines instead of growing the bubble
// arbitrarily wide.
const MAX_TEXT_WIDTH_PX = 220;

// The canvas below is built at this many real pixels per logical unit —
// independent of the renderer's own aa_mode/supersampling setting (that
// only affects the main WebGL canvas's render target, never a texture an
// app draws itself). Without this, the canvas was sized directly off
// FONT's own px metrics — maybe 150x50 real pixels — then stretched onto a
// sprite that can occupy several hundred *screen* pixels once the camera
// is close, magnifying every already-anti-aliased glyph edge into a
// blurry blob. Rendering at more real pixels up front is the actual fix;
// NearestFilter alone (kept below, for the border's chunky corners) can
// only ever preserve source detail, not add any.
const RENDER_SCALE = 3;

// World units [u] per logical (pre-RENDER_SCALE) canvas pixel — applied
// uniformly to both the canvas's own width and height, so a two-line
// message's text renders at the same real-world size as a one-line
// message's, rather than the whole sprite being squashed into a fixed
// on-screen height regardless of how many lines it grew to.
const WORLD_UNITS_PER_PX: f64 = 0.0072;

// Air gap between the character model's own top (ChatBubbleProps.anchor_z
// — its configured height_u) and the bubble's tail tip. Previously the
// caller (companion.tsx) passed `height_u * 1.15`, an arbitrary multiplier
// with no real relationship to any variant's actual head height; this
// makes the gap this component's own concern, as a fixed clearance added
// on top of wherever the model's bounding box actually ends.
const HEAD_CLEARANCE_U: f64 = 0.22;

/** Plus-shaped union of two overlapping rects — the corners left uncovered
 * read as a chunky, pixel-art "rounded" corner at any notch size, without
 * needing a real arc (which would anti-alias against Nearest sampling and
 * look wrong, not right, at this resolution). */
function fill_notched_rect(ctx: CanvasRenderingContext2D, x: number, y: number, w: number, h: number, notch: number, fill: string): void {
  ctx.fillStyle = fill;
  ctx.fillRect(x + notch, y, Math.max(0, w - notch * 2), h);
  ctx.fillRect(x, y + notch, w, Math.max(0, h - notch * 2));
}

/** Greedy word wrap against ctx's currently-set font, then a per-character
 * fallback pass for any line still too wide on its own — a single long
 * word, or (the common case that broke this the first time around) a
 * Korean message typed with no spacing between words at all, which the
 * word pass alone treats as one unbreakable "word" spanning the whole
 * message. */
function wrap_text(ctx: CanvasRenderingContext2D, text: string, max_width: number): string[] {
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

type BuiltBubble = { canvas: HTMLCanvasElement; logical_w: f64; logical_h: f64 };

function build_bubble_canvas(text: string): BuiltBubble {
  const measure_ctx = document.createElement("canvas").getContext("2d")!;
  measure_ctx.font = FONT;
  const lines = wrap_text(measure_ctx, text, MAX_TEXT_WIDTH_PX);
  const text_w = Math.ceil(Math.max(...lines.map((line) => measure_ctx.measureText(line).width)));

  const box_w = text_w + PADDING_X * 2;
  const box_h = lines.length * LINE_H + PADDING_Y * 2;
  const logical_h = box_h + TAIL_H;

  const canvas = document.createElement("canvas");
  canvas.width = box_w * RENDER_SCALE;
  canvas.height = logical_h * RENDER_SCALE;
  const ctx = canvas.getContext("2d")!;
  ctx.imageSmoothingEnabled = false;
  // Every draw call below stays in the same logical (box_w/box_h) units —
  // this scales the whole coordinate system up to the canvas's real pixel
  // count instead of hand-multiplying every constant by RENDER_SCALE.
  ctx.scale(RENDER_SCALE, RENDER_SCALE);

  fill_notched_rect(ctx, 0, 0, box_w, box_h, NOTCH, "#000");
  fill_notched_rect(ctx, BORDER, BORDER, box_w - BORDER * 2, box_h - BORDER * 2, Math.max(1, NOTCH - BORDER), "#fff");

  const tail_cx = box_w / 2;
  ctx.fillStyle = "#000";
  ctx.beginPath();
  ctx.moveTo(tail_cx - TAIL_W / 2, box_h - BORDER);
  ctx.lineTo(tail_cx + TAIL_W / 2, box_h - BORDER);
  ctx.lineTo(tail_cx, box_h + TAIL_H - BORDER);
  ctx.closePath();
  ctx.fill();
  ctx.fillStyle = "#fff";
  ctx.beginPath();
  ctx.moveTo(tail_cx - TAIL_W / 2 + BORDER * 1.5, box_h - BORDER);
  ctx.lineTo(tail_cx + TAIL_W / 2 - BORDER * 1.5, box_h - BORDER);
  ctx.lineTo(tail_cx, box_h + TAIL_H - BORDER * 2.5);
  ctx.closePath();
  ctx.fill();

  ctx.font = FONT;
  ctx.fillStyle = "#000";
  ctx.textAlign = "center";
  ctx.textBaseline = "middle";
  lines.forEach((line, i) => {
    ctx.fillText(line, box_w / 2, PADDING_Y + LINE_H * (i + 0.5));
  });

  return { canvas, logical_w: box_w, logical_h };
}

export type ChatBubbleProps = {
  /** Local Z (this character's own group frame) of the model's own top —
   * usually its configured height_u. The bubble adds its own clearance
   * above this and positions by its tail tip, not its center, so callers
   * don't need to know anything about the bubble's own current height. */
  anchor_z: f64;
};

export function ChatBubble(props: ChatBubbleProps) {
  const { anchor_z } = props;
  const chatLines = useUiStore((state) => state.chatLines);
  const last = chatLines.length > 0 ? chatLines[chatLines.length - 1] : null;

  const [visible_line, set_visible_line] = useState<{ id: number; text: string } | null>(null);

  // Driven by last.ts (wall-clock, set once when the line was appended) so
  // this stays correct across a remount — e.g. a character swap remounts
  // Walker under a new variant.key (companion.tsx), which would otherwise
  // reset local "have I already shown this one" state and make an old,
  // already-dismissed line look brand new again, with no way to tell it
  // apart from one that just arrived.
  useEffect(() => {
    if (!last) {
      set_visible_line(null);
      return;
    }
    const remaining = last.ts + SHOW_DURATION_MS - Date.now();
    if (remaining <= 0) {
      set_visible_line((current) => (current?.id === last.id ? null : current));
      return;
    }
    set_visible_line(last);
    const timer = setTimeout(() => {
      set_visible_line((current) => (current?.id === last.id ? null : current));
    }, remaining);
    return () => clearTimeout(timer);
  }, [last]);

  const built = useMemo(() => {
    if (!visible_line) return null;
    const { canvas, logical_w, logical_h } = build_bubble_canvas(visible_line.text);
    const texture = new THREE.CanvasTexture(canvas);
    texture.magFilter = THREE.NearestFilter;
    texture.minFilter = THREE.NearestFilter;
    texture.colorSpace = THREE.SRGBColorSpace;
    return { texture, width: logical_w * WORLD_UNITS_PER_PX, height: logical_h * WORLD_UNITS_PER_PX };
  }, [visible_line]);

  // CanvasTexture holds a real WebGL resource — useMemo alone doesn't clean
  // one up when a newer message replaces it (or the bubble is dismissed).
  useEffect(() => {
    if (!built) return;
    return () => built.texture.dispose();
  }, [built]);

  if (!built) return null;

  // Sprite position is its own center; the tail tip is its bottom edge, so
  // the center sits half the sprite's own height above the intended tip.
  const tip_z = anchor_z + HEAD_CLEARANCE_U;
  const sprite_z = tip_z + built.height / 2;

  return (
    <sprite position={[0, 0, sprite_z]} scale={[built.width, built.height, 1]} renderOrder={999}>
      {
        /* depthTest off + high renderOrder: a chat bubble reads as UI, not as
      a real object the terrain/buildings should be able to hide. */
      }
      <spriteMaterial map={built.texture} transparent depthTest={false} />
    </sprite>
  );
}
