// @pxl/web/shared/ui/toast/toaster.tsx
// Renders the toast list directly — no Sonner or similar library. Sonner's own
// positioning/stacking/swipe-dismiss fought this app's card-deck + swipe-to-rotate
// toast feature, so this owns layout end to end instead.

import type { u32 } from "@xpute/core/abi/word.ts";
import { memo, useEffect, useState } from "react";
import { getToastEntries, isToastRemoving, subscribeToastEntries, type ToastRenderFn } from "./store.ts";

const TOAST_EXTRA_LIFT_PX = 20;
const BOTTOM_SAFE_GAP_PX = 16;

interface ToasterProps {
  /** Whatever's already reserved at the bottom of the screen (e.g. a mode
   * bar) that this toast stack shouldn't overlap — this component has no
   * feature-specific knowledge of its own, the caller supplies it. */
  bottomPx?: number;
}

// Toaster re-renders on ANY entry changing (one shared subscription below), which
// re-invokes entries.map() for every toast, not just the one that changed. Without
// this memo boundary, an unrelated toast updating (e.g. the map-status-island toast,
// which ticks every few seconds) re-triggered every other toast's layout spring too,
// even ones nothing had actually happened to.
const ToastEntry = memo(function ToastEntry(
  { render, index, stackSize, isRemoving }: { render: ToastRenderFn; index: u32; stackSize: u32; isRemoving: boolean },
) {
  return <>{render({ index, stackSize, isRemoving })}</>;
});

const Toaster = ({ bottomPx = 0 }: ToasterProps) => {
  const [, forceRender] = useState(0);
  useEffect(() => subscribeToastEntries(() => forceRender((n) => n + 1)), []);

  // Not capped here — stackSize has to reflect the true queued count for toast.tsx's
  // rotation math (index/stackSize drive a modulo). The visible-count cap lives in
  // toast.tsx's own depth math instead; capping the array here would shrink the queue
  // itself instead of just what's drawn.
  const entries = getToastEntries();

  return (
    // Fixed, centered anchor — every entry below is absolutely positioned to this same
    // point. No per-toast measurement or sibling-derived stacking math: that
    // neighbor-dependent positioning is what broke repeatedly under Sonner.
    <div
      style={{
        position: "fixed",
        left: "50%",
        bottom: `calc(${bottomPx + BOTTOM_SAFE_GAP_PX + TOAST_EXTRA_LIFT_PX}px + var(--vv-bottom, 0px))`,
        transition: "bottom 150ms ease-out",
        transform: "translateX(-50%)",
        // Explicit z-index needed or this loses to the map overlay chrome around it
        // (mode bar z-40, address/compass cluster z-50) — renders fully correctly, just
        // invisibly behind them. Ladder: map chrome 40-50 < toast 60 < modal 70 (sheet.tsx).
        zIndex: 60,
        pointerEvents: "none",
      }}
    >
      {
        /**
         * left/right:0, no transform: `transform` on this per-entry wrapper creates a new
         * CSS stacking context that traps toast.tsx's own z-index inside it, so it can no
         * longer be compared against a different entry's z-index at all. Confirmed live:
         * sibling toasts fell back to DOM order for both paint AND pointer hit-testing,
         * so swiping the toast that looked frontmost landed on a different, visually-behind
         * one instead.
         */
      }
      {entries.map((entry, index) => (
        <div key={entry.id} style={{ position: "absolute", left: 0, right: 0, bottom: 0 }}>
          <ToastEntry render={entry.render} index={index} stackSize={entries.length} isRemoving={isToastRemoving(entry.id)} />
        </div>
      ))}
    </div>
  );
};

export { Toaster };
