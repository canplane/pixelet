// @pxl/web/render/gpu/badge_sheet.ts

/**
 * The badge sheet's pixels, painted from the kernel's draw specs
 * (crates/pxl/core/src/module/world/billboard/atlas.rs). The kernel packs the
 * sheet and says what each entry is — a place, a cluster count, a speech
 * bubble — and this canvas paints it: the whole sheet when the kernel's
 * generation moved (growth and compaction replace the sheet), else only the
 * entries drawn since, each through a scratch the size of one badge.
 *
 * Uploads are rectangles, not the sheet: re-uploading the whole canvas
 * whenever a badge was drawn cost 64 MB at 4096 px once for every frame a
 * badge was drawn in, and a phone paid it as a hundred-millisecond task each
 * time. The scratch is painted from the spec rather than copied out of the
 * sheet: reading a rectangle out of a large accelerated canvas has WebKit read
 * the whole canvas back into a pixel buffer — 64 MB of ArrayBuffer at 4096 px,
 * once per flush.
 */

import type { BranchCursor } from "@xpute/core/io/marshal/tree/mod.ts";
import { Cmd } from "@pxl/shared/spec/cmd.ts";

import { CORE_BUILD, packet } from "@/wasm/core_build.ts";
import type { BadgeSheet } from "./badges.ts";

/** atlas.rs's PAD: one transparent device pixel around every entry. */
const PAD = 1;
/** Supersampling: the renderer's device pixel ratio, clamped to 2. */
const RENDER_SCALE = 2;

/** Matches app/style/index.css's body font-family. */
const FONT_FAMILY = '-apple-system, BlinkMacSystemFont, "SF Pro Text", "SF Pro Display", "Pretendard Variable", system-ui, sans-serif';

// ---- a place and a cluster (crates/pxl/core/src/module/world/poi/poi_badge.rs) ----

const BADGE_FONT = `400 12px ${FONT_FAMILY}`;
const ICON_FONT = "14px sans-serif";
const ICON_SIZE = 20;
const GAP = 8;
const PAD_Y = 7;
const DOT_DIAM = 6;
const DOT_GAP = 4;
const DOT_BORDER = 1;
const PILL_H = ICON_SIZE + PAD_Y;
const PILL_R = PILL_H / 2;
const TEXT_X = PILL_R + ICON_SIZE / 2 + GAP;
const DOT_CY = PILL_H + DOT_GAP + DOT_DIAM / 2;

/** The pill every badge stands on: fully-rounded ends. */
function draw_pill(ctx: CanvasRenderingContext2D, logical_w: number): void {
  ctx.fillStyle = "rgba(20,20,26,0.72)";
  ctx.beginPath();
  ctx.roundRect(0, 0, logical_w, PILL_H, PILL_R);
  ctx.fill();
}

/** The anchor dot, centred under the pill, marking the ground point it floats above. */
function draw_anchor_dot(ctx: CanvasRenderingContext2D, logical_w: number): void {
  ctx.fillStyle = "rgba(255,255,255,0.6)";
  ctx.beginPath();
  ctx.arc(logical_w / 2, DOT_CY, DOT_DIAM / 2, 0, Math.PI * 2);
  ctx.fill();
  ctx.strokeStyle = "rgba(255,255,255,0.25)";
  ctx.lineWidth = DOT_BORDER;
  ctx.stroke();
}

function draw_place(ctx: CanvasRenderingContext2D, icon: string, color: string, name: string, logical_w: number): void {
  draw_pill(ctx, logical_w);
  // The icon circle is concentric with the pill's rounded left cap.
  const icon_cx = PILL_R;
  const icon_cy = PILL_H / 2;
  ctx.fillStyle = color;
  ctx.beginPath();
  ctx.arc(icon_cx, icon_cy, ICON_SIZE / 2, 0, Math.PI * 2);
  ctx.fill();
  ctx.font = ICON_FONT;
  ctx.textAlign = "center";
  ctx.textBaseline = "middle";
  ctx.fillText(icon, icon_cx, icon_cy + 1);
  ctx.font = BADGE_FONT;
  ctx.textAlign = "left";
  ctx.textBaseline = "middle";
  ctx.fillStyle = "#fff";
  ctx.fillText(name, TEXT_X, PILL_H / 2 + 1);
  draw_anchor_dot(ctx, logical_w);
}

function draw_cluster(ctx: CanvasRenderingContext2D, label: string, logical_w: number): void {
  draw_pill(ctx, logical_w);
  ctx.font = BADGE_FONT;
  ctx.textAlign = "center";
  ctx.textBaseline = "middle";
  ctx.fillStyle = "#fff";
  ctx.fillText(label, logical_w / 2, PILL_H / 2 + 1);
  draw_anchor_dot(ctx, logical_w);
}

// ---- a speech bubble (crates/pxl/core/src/module/world/character/chat_bubble.rs) ----

const BUBBLE_FONT = `600 14px ${FONT_FAMILY}`;
const PADDING_Y = 8;
const LINE_H = 17;
const BORDER = 2;
const NOTCH = 3;
const TAIL_W = 14;
const TAIL_H = 10;

/** Plus-shaped union of two overlapping rects — the corners left uncovered read
 * as a chunky, pixel-art "rounded" corner without a real arc, which would
 * anti-alias against Nearest sampling. */
function fill_notched_rect(ctx: CanvasRenderingContext2D, x: number, y: number, w: number, h: number, notch: number, fill: string): void {
  ctx.fillStyle = fill;
  ctx.fillRect(x + notch, y, Math.max(0, w - notch * 2), h);
  ctx.fillRect(x, y + notch, w, Math.max(0, h - notch * 2));
}

function draw_bubble(ctx: CanvasRenderingContext2D, lines: readonly string[], box_w: number, box_h: number): void {
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
  ctx.font = BUBBLE_FONT;
  ctx.fillStyle = "#000";
  ctx.textAlign = "center";
  ctx.textBaseline = "middle";
  lines.forEach((line, i) => ctx.fillText(line, box_w / 2, PADDING_Y + LINE_H * (i + 0.5)));
}

// ---- the sheet ----

type Entry = { x: number; y: number; w: number; h: number; kind: number; icon: string; color: string; text: string; logical_w: number; box_h: number };

/** Paints an entry with its padded rectangle's corner at (ox, oy). */
function paint(ctx: CanvasRenderingContext2D, e: Entry, ox: number, oy: number): void {
  ctx.save();
  ctx.translate(ox + PAD, oy + PAD);
  ctx.imageSmoothingEnabled = false;
  ctx.scale(RENDER_SCALE, RENDER_SCALE);
  if (e.kind === 0) draw_place(ctx, e.icon, e.color, e.text, e.logical_w);
  else if (e.kind === 1) draw_cluster(ctx, e.text, e.logical_w);
  else draw_bubble(ctx, e.text.split("\n"), e.logical_w, e.box_h);
  ctx.restore();
}

class KernelSheet implements BadgeSheet {
  private canvas: HTMLCanvasElement | null = null;
  private ctx: CanvasRenderingContext2D | null = null;
  private scratch: HTMLCanvasElement | null = null;
  private scratch_ctx: CanvasRenderingContext2D | null = null;

  flush_into(generation: number, whole: (canvas: HTMLCanvasElement, size: number) => void, part: (scratch: HTMLCanvasElement, w: number, h: number, x: number, y: number) => void): number {
    const { replaced, size, next, entries } = CORE_BUILD.call(Cmd.WORLD_SHEET, packet((b) => b.u32(generation)), (_, reply) => {
      const [replaced, size, next, rows] = reply!.get<[boolean, number, number, BranchCursor]>();
      const entries = rows.get<BranchCursor[]>().map((r): Entry => {
        const [x, y, w, h, kind, icon, color, text, logical_w, box_h] = r.get<[number, number, number, number, number, string, string, string, number, number]>();
        return { x, y, w, h, kind, icon, color, text, logical_w, box_h };
      });
      return { replaced, size, next, entries };
    });
    if (replaced) {
      if (!this.canvas) {
        this.canvas = document.createElement("canvas");
        this.ctx = this.canvas.getContext("2d")!;
      }
      const canvas = this.canvas, ctx = this.ctx!;
      canvas.width = size;
      canvas.height = size;
      ctx.clearRect(0, 0, size, size);
      for (const e of entries) paint(ctx, e, e.x, e.y);
      whole(canvas, size);
      return next;
    }
    if (entries.length > 0 && !this.scratch) {
      this.scratch = document.createElement("canvas");
      this.scratch_ctx = this.scratch.getContext("2d")!;
    }
    for (const e of entries) {
      const scratch = this.scratch!, ctx = this.scratch_ctx!;
      if (scratch.width < e.w || scratch.height < e.h) {
        scratch.width = Math.max(scratch.width, e.w);
        scratch.height = Math.max(scratch.height, e.h);
      }
      ctx.clearRect(0, 0, e.w, e.h);
      paint(ctx, e, 0, 0);
      part(scratch, e.w, e.h, e.x, e.y);
    }
    return next;
  }
}

/** The one sheet: the kernel's billboard atlas, mirrored. */
export const KERNEL_SHEET: BadgeSheet = new KernelSheet();
