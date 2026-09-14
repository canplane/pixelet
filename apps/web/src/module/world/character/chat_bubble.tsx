// @pxl/web/module/world/character/chat_bubble.tsx

/**
 * A billboarded speech bubble over the companion's head, showing the most
 * recent line from the (currently dev-only) map chat — see
 * module/map/ui/overlay/mode_bar.tsx / dummy_chat_text.tsx for where
 * chatLines comes from. World-anchored-billboard tier (see
 * issue/20260907-world-rendering-tiers.md); the actual render is
 * a billboard set of the module standing on the companion's actor
 * (crates/pxl/core/src/world/billboard.rs), drawn from the sheet POI badges share.
 * This file owns only what's chat-bubble-specific: reading chatLines, the
 * show/expire timer, and drawing the notched speech-bubble shape.
 *
 * The bubble is drawn on a plain 2D canvas (notched-corner border, tail)
 * rather than an image asset, since its size has to track the message text
 * anyway.
 */

import { useEffect, useRef, useState } from "react";

import type { f64, u32 } from "@xpute/core/kit/word.ts";
import { useUiStore } from "@/app/store/ui_store.ts";
import { wrap_text } from "../billboard/canvas_text.ts";
import { FONT_FAMILY, FONT_WEIGHT_BOLD } from "../billboard/canvas_scale.ts";
import { BILLBOARD_ORDER } from "../billboard/sheet.ts";
import { NO_SLOT } from "../billboard/atlas.ts";
import { billboard_sheet } from "../billboard/sheet.ts";
import { useTick } from "@/module/world/sched/tick.ts";
import { CORE_PXL, FIXED_ALLOC } from "@/wasm/core_pool.ts";

// A new chat line shows for this long, then the bubble disappears — chat
// itself has no such expiry (dummy_chat_text.tsx's log keeps up to 10
// lines forever), so this is tracked locally here, not in ui_store.
const SHOW_DURATION_MS = 8000;

const FONT = `${FONT_WEIGHT_BOLD} 14px ${FONT_FAMILY}`;
const PADDING_X = 11;
const PADDING_Y = 8;
const LINE_H = 17;
const BORDER = 2;
const NOTCH = 3;
const TAIL_W = 14;
const TAIL_H = 10;

// A long message wraps onto more lines instead of growing the bubble
// arbitrarily wide.
const MAX_TEXT_WIDTH_PX = 220;

// The canvas below is built at this many real pixels per logical unit, and
// it should equal the renderer's own device pixel ratio.
//
// The tick clamps that to post_cfg.DPR, [1, 2] (sched/tick.ts), and
// distance_fade_far returns at most 1, so a billboard's drawn size never
// exceeds its logical size in CSS pixels — the fade only ever shrinks it, so
// two is the exact requirement rather than a guess.
//
// It was 3, chosen against the worry that a sprite close to the camera
// occupies many more screen pixels than the canvas has. It does not: the
// fade holds the sprite at a constant *screen* size, which is what the
// world size grows to preserve. Three was 2.25x the pixels for nothing, and
// arguably worse — magFilter is Nearest for the up-close pixel read, and
// drawing at 3 to display at 2 is a minification through the mipmap chain.
const RENDER_SCALE = 2;

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

/** A message laid out: its wrapped lines and the bubble's logical size. */
type BubbleLayout = { lines: string[]; box_w: f64; box_h: f64; logical_h: f64 };

function layout_bubble(text: string): BubbleLayout {
  const measure_ctx = document.createElement("canvas").getContext("2d")!;
  measure_ctx.font = FONT;
  const lines = wrap_text(measure_ctx, text, MAX_TEXT_WIDTH_PX);
  const text_w = Math.ceil(Math.max(...lines.map((line) => measure_ctx.measureText(line).width)));
  const box_w = text_w + PADDING_X * 2;
  const box_h = lines.length * LINE_H + PADDING_Y * 2;
  return { lines, box_w, box_h, logical_h: box_h + TAIL_H };
}

/** Draws the bubble with its top-left at the context's origin, in real
 * pixels RENDER_SCALE to the logical one. */
function draw_bubble(ctx: CanvasRenderingContext2D, layout: BubbleLayout): void {
  const { lines, box_w, box_h } = layout;
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
}

export type ChatBubbleProps = {
  /** Local Z (this character's own group frame) of the model's own top —
   * usually its configured height_u. The bubble adds its own clearance
   * above this and positions by its tail tip, not its center, so callers
   * don't need to know anything about the bubble's own current height. */
  anchor_z: f64;
  /** The character's actor, once it exists. */
  actor: u32 | null;
};

/** Floats a billboard instance takes (crates/pxl/core/src/world/billboard.rs). */
const BILLBOARD_FLOATS = 10;

export function ChatBubble(props: ChatBubbleProps) {
  const { anchor_z, actor } = props;
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

  // The tail tip is the anchor, at the model's top plus the clearance.
  useBubbleSet(actor, visible_line?.text ?? null, anchor_z + HEAD_CLEARANCE_U);
  return null;
}

/**
 * Drawn into the shared sheet, and one instance of a billboard set that
 * follows `actor` at `tip_z` — so the module places it wherever the actor
 * stands each tick. The instance is written again when the sheet is
 * replaced, which moves its rectangle.
 */
function useBubbleSet(actor: u32 | null, text: string | null, tip_z: f64): void {
  const [held, set_held] = useState<{ set: u32; slot: u32; layout: BubbleLayout } | null>(null);

  useEffect(() => {
    const p = CORE_PXL;
    if (!p || actor === null || text === null) return;
    const layout = layout_bubble(text);
    const slot = billboard_sheet.add(layout.box_w * RENDER_SCALE, layout.logical_h * RENDER_SCALE, (ctx) => draw_bubble(ctx, layout));
    if (slot === NO_SLOT) return;
    const set = p.pxl_billboards_new(BILLBOARD_ORDER.chat);
    set_held({ set, slot, layout });
    return () => {
      p.pxl_billboards_remove(set);
      billboard_sheet.release(slot);
      set_held(null);
    };
  }, [actor, text]);

  const written = useRef<{ held: typeof held; tip_z: f64; generation: u32 }>({ held: null, tip_z: 0, generation: 0 });
  useTick("cosmetic", () => {
    const p = CORE_PXL;
    const w = written.current;
    if (!p || !held || actor === null) return;
    if (w.held === held && w.tip_z === tip_z && w.generation === billboard_sheet.sheet_generation) return;
    w.held = held;
    w.tip_z = tip_z;
    w.generation = billboard_sheet.sheet_generation;
    const { set, slot, layout } = held;
    const lane = FIXED_ALLOC.take(Float32Array, BILLBOARD_FLOATS);
    // The tail tip is the anchor, at the bottom of the sprite.
    lane.set([0, 0, 0, layout.box_w, layout.logical_h, billboard_sheet.u0(slot), billboard_sheet.v0(slot), billboard_sheet.u1(slot), billboard_sheet.v1(slot), 1]);
    p.pxl_billboards_follow(set, actor, tip_z);
    p.pxl_billboards_write(set, 1, lane.byteOffset);
    FIXED_ALLOC.give(lane);
  }, "chat_bubble");
}
