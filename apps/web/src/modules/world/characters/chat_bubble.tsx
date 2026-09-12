// @pxl/web/modules/world/characters/chat_bubble.tsx

/**
 * A billboarded speech bubble over the companion's head, showing the most
 * recent line from the (currently dev-only) map chat — see
 * modules/map/ui/overlay/mode_bar.tsx / dummy_chat_text.tsx for where
 * chatLines comes from. World-anchored-billboard tier (see
 * discussions/20260907-world-rendering-tiers.md); the actual render is
 * billboard/billboard.tsx's `<Billboard>`, shared with poi_layer.tsx. This
 * file owns only what's chat-bubble-specific: reading chatLines, the
 * show/expire timer, and drawing the notched speech-bubble shape onto a
 * canvas.
 *
 * The bubble is drawn on a plain 2D canvas (notched-corner border, tail)
 * rather than an image asset, since its size has to track the message text
 * anyway.
 */

import { useEffect, useMemo, useState } from "react";

import type { f64 } from "@xpute/core/kit/word.ts";
import { useUiStore } from "@/app/store/ui_store.ts";
import { wrap_text } from "../billboard/canvas_text.ts";
import { FONT_FAMILY, FONT_WEIGHT_BOLD } from "../billboard/canvas_scale.ts";
import { Billboard, BILLBOARD_ORDER, type BuiltBillboard } from "../billboard/billboard.tsx";
import { make_canvas_texture } from "../billboard/canvas_texture.ts";

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
// scene.tsx caps that at 2 (`dpr={[1, 2]}`)
// and distance_fade_far returns at most 1, so a billboard's drawn size never
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

function build_bubble_canvas(text: string): BuiltBillboard {
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

  // The whole bubble: unlike a POI badge's anchor dot, every part of this
  // is message rather than marker.
  return { canvas, texture: make_canvas_texture(canvas), logical_w: box_w, logical_h, center_y: logical_h, collide_h: logical_h };
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

  const built = useMemo(() => (visible_line ? build_bubble_canvas(visible_line.text) : null), [visible_line]);
  // A bubble's canvas is rebuilt per distinct message rather than cached, so
  // its texture is this component's to release — unlike a POI badge, which
  // is held by id and disposed when the badge is evicted.
  useEffect(() => () => built?.texture.dispose(), [built]);

  // tip_z is the tail's fixed bottom edge (Billboard's own base_z).
  const tip_z = anchor_z + HEAD_CLEARANCE_U;
  return <Billboard built={built} base_z={tip_z} render_order={BILLBOARD_ORDER.chat} />;
}
