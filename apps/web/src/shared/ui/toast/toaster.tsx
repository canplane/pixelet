// @pxl/web/shared/ui/toast/toaster.tsx
// Renders the toast list directly — no Sonner or similar library. Sonner's
// own positioning/stacking/swipe-dismiss kept fighting this app's card-deck
// + swipe-to-rotate toast feature, so this owns layout end to end instead.

import { memo, useEffect, useState } from "react";
import { BOTTOM_OVERLAY_SAFE_PX } from "@/shared/ui/overlay_safe_area.ts";
import { get_toast_entries, is_toast_removing, subscribe_toast_entries, type ToastRenderFn } from "./store.ts";

/** Extra lift on top of BOTTOM_OVERLAY_SAFE_PX (which only guarantees
 * clearing the mode bar) — the toast sat right at that floor, which
 * read as too close to the mode bar for comfortable breathing room. */
const TOAST_EXTRA_LIFT_PX = 20;

/**
 * Toaster re-renders on ANY entry changing (one shared subscription — see
 * below), which means entries.map() re-invokes every OTHER toast's own
 * render() too, not just the one that actually changed. Without this memo
 * boundary, an unrelated toast (e.g. the map-status-island toast, which can
 * legitimately update every few seconds on its own) spuriously re-rendered
 * every other toast on screen right along with it — including ones whose
 * own props hadn't changed at all — which was enough to re-trigger the
 * pill's own layout spring for toasts nothing had actually happened to. A
 * general list-rendering mechanism re-stimulating every item over one
 * specific item's update is exactly backwards; each toast should only
 * re-render when ITS OWN index/stack_size/is_removing/render reference
 * actually changes.
 */
const ToastEntry = memo(function ToastEntry(
  { render, index, stack_size, is_removing }: { render: ToastRenderFn; index: number; stack_size: number; is_removing: boolean },
) {
  return <>{render({ index, stack_size, is_removing })}</>;
});

const Toaster = () => {
  const [, force_render] = useState(0);
  useEffect(() => subscribe_toast_entries(() => force_render((n) => n + 1)), []);

  // Full queue, not capped here — stack_size below has to reflect the TRUE
  // count for toast.tsx's rotation math (index/stack_size drive a modulo)
  // to cycle through everything that's actually queued. The visible-count
  // cap is a rendering concern of toast.tsx's own depth math instead (it
  // forces opacity to 0 once an entry's rotated depth falls past its own
  // MAX_STACK_DEPTH, rather than unmounting it — see its own doc comment
  // for why never unmounting matters) — capping the array here would
  // silently shrink the queue itself instead of just what's drawn.
  const entries = get_toast_entries();

  return (
    // Fixed, centered anchor — every toast entry below is itself absolutely
    // positioned to this SAME point (bottom:0, centered), regardless of how
    // many exist or which are mid-removal. No per-toast measurement, no
    // margin/stacking math derived from siblings: that neighbor-dependent
    // positioning is exactly what broke repeatedly under Sonner.
    <div
      style={{
        position: "fixed",
        left: "50%",
        bottom: `${BOTTOM_OVERLAY_SAFE_PX + TOAST_EXTRA_LIFT_PX}px`,
        transform: "translateX(-50%)",
        // without an explicit z-index this defaults to auto (effectively
        // 0), which loses to the map overlay chrome around it (mode
        // bar z-40, address/compass cluster z-50) — the toast rendered
        // with fully correct opacity/position/content, just invisibly
        // behind those. Comfortably above every z-* already in use here.
        zIndex: 1000,
        // ignore pointer events so the map underneath stays interactive;
        // each toast's own pill re-enables pointer-events on itself
        pointerEvents: "none",
      }}
    >
      {
        /**
         * left/right:0 (full width), no transform — `transform` on this
         * per-entry wrapper (even just for horizontal centering) creates a
         * brand new CSS stacking context, which TRAPS whatever z-index
         * toast.tsx sets on its own root entirely inside that one wrapper:
         * it can no longer be compared against a *different* entry's
         * z-index at all, since stacking contexts are isolated from each
         * other by design. With that, sibling toasts stacked purely by DOM
         * order instead (last-rendered on top, in BOTH paint order and
         * pointer hit-testing) — confirmed live: swiping the toast that
         * visually looked frontmost was silently landing pointerdown/up on
         * a totally different (older, visually-behind) toast instead.
         * toast.tsx's own root already centers itself horizontally via
         * flexbox (`justify-center` on a `w-full` container), so this
         * wrapper doesn't need a transform for that at all.
         */
      }
      {entries.map((entry, index) => (
        <div key={entry.id} style={{ position: "absolute", left: 0, right: 0, bottom: 0 }}>
          <ToastEntry render={entry.render} index={index} stack_size={entries.length} is_removing={is_toast_removing(entry.id)} />
        </div>
      ))}
    </div>
  );
};

export { Toaster };
