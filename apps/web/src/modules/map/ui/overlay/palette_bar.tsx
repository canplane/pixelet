// @pxl/web/modules/map/ui/overlay/palette_bar.tsx

import { useState } from "react";
import { motion } from "framer-motion";
import { Hand, MessageCircle, Palette } from "lucide-react";

import { cn } from "@/shared/lib/utils/cn.ts";

const PALETTE_COLORS = [
  { id: "red", className: "bg-red-500" },
  { id: "blue", className: "bg-blue-500" },
  { id: "green", className: "bg-green-500" },
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
 * as one continuous pill across sizes: collapsed is w-14 h-12 (wider than
 * tall, so rounded-full renders a stadium, not a circle), expanded goes
 * much wider — one shape rule for both, matching this app's other circular
 * controls (compass/GPS).
 *
 * `layout` (framer-motion) turns the width/height swap into a smooth spring
 * resize instead of an instant snap.
 *
 * Nothing here is wired to a real chat/drawing system yet — mode, tool, and
 * message text are all local, cosmetic-only state.
 */
export function PaletteBar() {
  const [mode, set_mode] = useState<ModeId>("palette");
  const [tool, set_tool] = useState<ToolId>("pointer");
  const [message_text, set_message_text] = useState("");

  const message_expanded = mode === "message";
  const palette_expanded = mode === "palette";

  return (
    // gap-3 (was gap-2) — a little more separation between the two islands.
    <div className="flex items-center gap-3">
      <motion.div
        layout
        transition={SPRING}
        // h-12 fixed on both pills (here and below) — height was otherwise
        // whatever each one's own content needed, which could make the
        // pair visibly bob up/down by a pixel or two as either one's
        // content swapped. Locking both to the same height keeps them
        // level regardless of which is expanded, and also gives the
        // collapsed state the exact square footprint rounded-full needs
        // to read as a circle (see this component's own doc comment).
        className={cn(
          "flex items-center h-12 bg-white border border-white/15 rounded-full shadow-lg shadow-black/40",
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
              className="flex items-center justify-center w-14 h-12 rounded-full cursor-pointer text-black/50 hover:text-black/80 transition-colors shrink-0"
              title="message"
            >
              <MessageCircle className="w-4 h-4" strokeWidth={2} />
            </button>
          )
          : (
            <div className="flex items-center gap-2">
              <div className="flex items-center justify-center w-8 h-8 shrink-0">
                <MessageCircle className="w-4 h-4 text-black/50" strokeWidth={2} />
              </div>
              <input
                value={message_text}
                onChange={(e) => set_message_text(e.target.value)}
                placeholder="메시지 입력..."
                autoFocus
                className="bg-transparent text-sm text-black placeholder:text-black/40 outline-none w-32"
              />
            </div>
          )}
      </motion.div>

      <motion.div
        layout
        transition={SPRING}
        className={cn(
          "flex items-center h-12 bg-neutral-900 border border-white/15 rounded-full shadow-lg shadow-black/40",
          // px-2 — see the matching comment on the message pill above.
          palette_expanded && "px-2",
        )}
      >
        {!palette_expanded
          ? (
            <button
              type="button"
              onClick={() => set_mode("palette")}
              className="flex items-center justify-center w-14 h-12 rounded-full cursor-pointer text-white/60 hover:text-white/90 transition-colors shrink-0"
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
              isn't what was asked). p-2 (not h-8 alone) so BOTH axes get
              the same 8px inset in one step: dots are w-4 h-4 (16px), 16 +
              8 + 8 = 32 lands the whole cluster's box on the hand button's
              own 32px footprint, top/bottom and left/right alike. */
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
