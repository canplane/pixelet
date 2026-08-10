// @pxl/web/modules/map/ui/overlay/mode_bar.tsx

import { useState } from "react";
import { motion } from "framer-motion";
import { Hand, MessageCircle, Palette } from "lucide-react";

import { cn } from "@/shared/lib/utils/cn.ts";

const PALETTE_COLORS = [
  { id: "red", className: "bg-red-500" },
  { id: "orange", className: "bg-orange-500" },
  { id: "yellow", className: "bg-yellow-500" },
  { id: "green", className: "bg-green-500" },
  { id: "blue", className: "bg-blue-500" },
  { id: "indigo", className: "bg-indigo-500" },
  { id: "purple", className: "bg-purple-500" },
] as const;

type ToolId = "pointer" | (typeof PALETTE_COLORS)[number]["id"];
type ModeId = "message" | "palette";

const SPRING = { type: "spring" as const, stiffness: 500, damping: 35, mass: 0.6 };

/**
 * CONCEPT ONLY — two separate floating pills (message white, palette dark),
 * not one fused bar. Tapping a collapsed icon morphs that pill wider while
 * the other (if expanded) morphs back to icon-only — tried a single
 * shared-outline bar with a color seam first, read as ambiguous.
 *
 * `rounded-full` on both states (not a fixed radius) is what makes it read
 * as one continuous pill across sizes: collapsed is w-12 h-12 (a true circle,
 * matching this app's other circular controls like compass/GPS), expanded
 * goes much wider — one shape rule for both.
 *
 * `layout` (framer-motion) turns the width/height swap into a smooth spring
 * resize instead of an instant snap.
 *
 * Nothing here is wired to a real chat/drawing system yet — mode, tool, and
 * message text are all local, cosmetic-only state. Named for what it does
 * (switches between modes — message, palette, and whatever else lands here
 * later) rather than "palette bar", which only ever described one of its
 * two pills.
 */
export function ModeBar() {
  const [mode, set_mode] = useState<ModeId>("message");
  const [tool, set_tool] = useState<ToolId>("pointer");
  const [message_text, set_message_text] = useState("");

  const message_expanded = mode === "message";
  const palette_expanded = mode === "palette";

  return (
    // gap-3 (was gap-2) — a little more separation between the two islands.
    // min-w-0 so this row (and the message pill inside it, via its own
    // min-w-0 below) can actually shrink below its content's natural
    // width instead of overflowing once the input gets wide — flex items
    // default to min-width:auto, which blocks shrinking below content
    // size no matter how little room the parent actually has.
    //
    // justify-center: this row is w-full (spans the whole space
    // bottom_overlays.tsx reserves), but with neither pill given
    // flex-grow by default, plain flex layout would leave them sitting
    // flush-left in that now-wide invisible box instead of centered as a
    // compact pair — this keeps the collapsed look identical to before
    // w-full existed here.
    <div className="flex items-center justify-center gap-3 min-w-0 w-full">
      <motion.div
        layout
        transition={SPRING}
        // borderRadius via style, not a rounded-full class — this element
        // has `layout`, so framer-motion animates its width change with a
        // scale() transform under the hood (FLIP), which visually distorts
        // a *static* CSS border-radius into an oval whenever width and
        // height scale by different amounts (exactly this pill's case:
        // height is constant, width swings widely). framer-motion only
        // auto-corrects for that distortion when the radius comes through
        // its own `style` prop (a value it's actively tracking) — a
        // Tailwind class is invisible to it. toast.tsx's pill already does
        // this the same way (style={{ borderRadius: "999px" }}).
        style={{ borderRadius: "999px" }}
        // h-12 fixed on both pills (here and below) — height was otherwise
        // whatever each one's own content needed, which could make the
        // pair visibly bob up/down by a pixel or two as either one's
        // content swapped. Locking both to the same height keeps them
        // level regardless of which is expanded, and also gives the
        // collapsed state the exact square footprint needed to read as a
        // circle (see this component's own doc comment).
        // min-w-0: see the root row's own comment — this pill is the one
        // that should give way when space is tight, not the palette pill.
        // No flex-grow (deliberately) — the pill's natural size is
        // whatever its content (the w-96 input below) needs; it should
        // only ever get SMALLER than that under pressure (min-w-0 +
        // default flex-shrink), never bigger just because the row has
        // leftover space. flex-1 here was tried and overshot: it claimed
        // the entire row regardless of the input's own max-width, which
        // is what stretched the whole pill edge-to-edge.
        className={cn(
          "flex items-center h-12 bg-white border border-white/15 shadow-lg shadow-black/40 min-w-0",
          // px-2 — matches the ~8px vertical inset h-12 already gives the
          // tallest expanded-row content (the w-8/h-8 hand button) via
          // items-center centering, so left/right edge spacing reads the
          // same as top/bottom instead of the wider px-4 this used to be,
          // which sat visibly looser than the vertical gap.
          message_expanded && "px-2",
        )}
      >
        {!message_expanded
          ? (
            <button
              type="button"
              onClick={() => set_mode("message")}
              // w-12 (not w-14): equal to h-12 so rounded-full renders a true
              // circle when collapsed, matching this app's other circular
              // controls (CircularButton) instead of a stadium/half-pill shape.
              className="flex items-center justify-center w-12 h-12 rounded-full cursor-pointer text-black/50 hover:text-black/80 transition-colors shrink-0"
              title="message"
            >
              <MessageCircle className="w-4 h-4" strokeWidth={2} />
            </button>
          )
          : (
            <div className="flex items-center gap-2 min-w-0">
              <div className="flex items-center justify-center w-8 h-8 shrink-0">
                <MessageCircle className="w-4 h-4 text-black/50" strokeWidth={2} />
              </div>
              {
                /* w-96 (not w-full): 384px is this input's actual
                preferred size, not a lower bound to grow past — max-w-full
                min-w-0 is what lets it shrink under a narrower row, but
                nothing here should ever push it, or the pill around it,
                wider than 384px just because extra space exists. */
              }
              <input
                value={message_text}
                onChange={(e) => set_message_text(e.target.value)}
                placeholder="메시지 입력..."
                autoFocus
                className="bg-transparent text-sm text-black placeholder:text-black/40 outline-none w-96 max-w-full min-w-0"
              />
            </div>
          )}
      </motion.div>

      <motion.div
        layout
        transition={SPRING}
        // borderRadius via style — see the matching comment on the message
        // pill above for why a rounded-full class doesn't survive this
        // element's own layout-driven width change without distorting.
        style={{ borderRadius: "999px" }}
        // shrink-0: the palette pill stays full size regardless — the
        // message pill above is what gives way when the row is squeezed.
        className={cn(
          "flex items-center h-12 bg-neutral-900 border border-white/15 shadow-lg shadow-black/40 shrink-0",
          // px-2 — see the matching comment on the message pill above.
          palette_expanded && "px-2",
        )}
      >
        {!palette_expanded
          ? (
            <button
              type="button"
              onClick={() => set_mode("palette")}
              // w-12: see the matching comment on the message pill's collapsed button above.
              className="flex items-center justify-center w-12 h-12 rounded-full cursor-pointer text-white/60 hover:text-white/90 transition-colors shrink-0"
              title="palette"
            >
              <Palette className="w-4 h-4" strokeWidth={2} />
            </button>
          )
          : (
            <div className="flex items-center gap-1">
              <button
                type="button"
                onClick={() => set_tool("pointer")}
                className={cn(
                  "flex items-center justify-center w-8 h-8 rounded-full cursor-pointer transition-colors shrink-0",
                  // white/20 (not blue) — the very first version of this bar
                  // used a neutral white highlight for the selected tool;
                  // blue crept in during a later pass and read as an
                  // inconsistent one-off accent color for a concept piece
                  // that's otherwise entirely grayscale except the color
                  // swatches themselves.
                  tool === "pointer" ? "bg-white/20 text-white" : "text-white/60 hover:text-white/90",
                )}
                title="포인터"
              >
                <Hand className="w-4 h-4" strokeWidth={2} />
              </button>

              <div className="w-px h-5 bg-white/15 mx-1" />

              {
                /* Padding on the group as a single unit (not a box per
              dot — that widened the gap between dots themselves, which
              isn't what was asked). p-2 (not h-8 alone) so both axes get
              the same 8px inset in one step: dots are w-4 h-4 (16px), 16 +
              8 + 8 = 32 matches the hand button's own 32px height exactly
              — width just grows with however many colors there are (7 now). */
              }
              <div className="flex items-center gap-2 p-2">
                {PALETTE_COLORS.map((c) => (
                  <button
                    key={c.id}
                    type="button"
                    onClick={() => set_tool(c.id)}
                    className={cn(
                      "w-4 h-4 rounded-full cursor-pointer transition-transform shrink-0",
                      c.className,
                      tool === c.id ? "ring-1 ring-white ring-offset-1 ring-offset-neutral-900 scale-110" : "scale-100",
                    )}
                    title={c.id}
                  />
                ))}
              </div>
            </div>
          )}
      </motion.div>
    </div>
  );
}
