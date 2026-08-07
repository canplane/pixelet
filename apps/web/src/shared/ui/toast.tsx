// @pxl/web/shared/ui/toast.tsx

import { useEffect, useMemo, useRef, useState } from "react";
import type { ReactNode } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { useSonner } from "sonner";
import { cn } from "@/shared/lib/utils/cn.ts";

/** Single source of truth for the pill's own fixed height and how much of a
 * stacked-behind toast should peek out below it — sonner.tsx's margin math
 * has to use these exact same numbers to line up with what's actually
 * rendered here. */
export const TOAST_HEIGHT_PX = 64;
export const TOAST_STACK_PEEK_PX = 6;

/** How many cards deep the deck visually goes before further-back toasts
 * just stay fully hidden at the last card's own depth (still logically
 * present/counted, just not drawn as yet another visible sliver). */
export const MAX_STACK_DEPTH = 2;

/** Per-depth opacity/scale, front (depth 0) to back (MAX_STACK_DEPTH). */
const STACK_OPACITY = [1, 0.6, 0.3];
const STACK_SCALE = [1, 0.94, 0.88];

type ToastProps = {
  /** Same id passed to toast.custom()'s own options — needed to look this
   * toast's own position up in useSonner()'s live list below. */
  id: string;
  children?: ReactNode;
  icon?: ReactNode;
  title: string;
  description?: string;
  subDescription?: string;
  statusColor?: string;
  className?: string;
  backgroundColor?: string;
};

export function Toast({
  id,
  icon,
  title,
  description,
  subDescription,
  statusColor,
  className,
  backgroundColor = "bg-[#FFFDF7]/85",
}: ToastProps) {
  const isExpanded = !!(title || subDescription);
  const hasSub = !!subDescription;

  /**
   * Multiple toast ids can legitimately be up at once — this reads as a
   * card deck: the newest takes the one fixed front slot, and the previous
   * occupant slides back a depth, fading/shrinking more per level. Sonner
   * prepends new toasts, so index 0 is always most recent — stack_index is
   * depth in the deck.
   *
   * Driven through React/framer-motion instead of a CSS [data-front]
   * selector on Sonner's own <li> — that fought Sonner's own
   * higher-specificity stacking rule (and Tailwind's layered !important)
   * and never won. Styling our own rendered element sidesteps the fight
   * entirely.
   */
  const { toasts } = useSonner();
  const raw_stack_index = toasts.findIndex((t) => t.id === id);
  // cap the visual depth at MAX_STACK_DEPTH without capping stack_index
  // itself, which sonner.tsx's margin math still needs uncapped
  const stack_index = Math.max(0, raw_stack_index);
  const stack_depth_raw = Math.min(stack_index, MAX_STACK_DEPTH);

  /**
   * Sonner intends to fade its own <li> out via CSS before unmounting, but
   * that depends on custom properties computed for its native
   * absolute-position stacking system — the same system whose [data-front]
   * rule never worked once this toast switched to position:static, so the
   * removal fade breaks the same way: nothing animates, it just vanishes.
   * Worse, Sonner hardcodes a 200ms unmount timer after marking a toast
   * removed, so no exit animation here can run longer than that regardless.
   *
   * useSonner()'s toasts list drops a removed entry immediately, so
   * raw_stack_index reads -1 as soon as dismissal starts. Rather than
   * freezing opacity/scale at their last value (which doesn't animate
   * anything, just stops the snap-back), actively animate to fully
   * transparent/shrunk here — short enough (150ms) to finish inside
   * Sonner's 200ms deadline, a real fade this component owns end to end.
   */
  const last_known_depth = useRef(stack_depth_raw);
  if (raw_stack_index !== -1) last_known_depth.current = stack_depth_raw;
  const is_removing = raw_stack_index === -1;
  const stack_depth = is_removing ? last_known_depth.current : stack_depth_raw;

  const springConfig = useMemo(
    () => ({
      type: "spring" as const,
      stiffness: 500,
      // 12 -> 30: at the lower value the auto-width layout could overshoot
      // on expand and visibly glitch.
      damping: 30,
      mass: 0.5,
    }),
    [],
  );

  const [wasExpanded, setWasExpanded] = useState(false);

  useEffect(() => {
    if (isExpanded) {
      const timer = setTimeout(() => setWasExpanded(true), 450);
      return () => clearTimeout(timer);
    } else {
      setWasExpanded(false);
    }
  }, [isExpanded]);

  // 80 -> 64: shrunk after it read as oversized next to the circular buttons (w-11/w-14 = 44/56px) it sits alongside
  const containerHeight = TOAST_HEIGHT_PX;

  return (
    // wrapper: the box size Sonner actually sees as "the toast" — fixing
    // its height keeps Sonner's layout stable regardless of the pill inside
    <motion.div
      /**
       * New arrivals fade in rather than popping to their settled opacity —
       * `initial` only applies on first mount, so a toast already on screen
       * that's just getting demoted to a deeper depth animates from its
       * current values via animate/transition only. No y-offset here:
       * Sonner's own <li> already slides up from translateY(100%) on mount
       * via its own CSS, so a second independent y-animation on top of that
       * read as a glitchy "flickers low, then jumps up".
       */
      initial={{ opacity: 0 }}
      // is_removing overrides straight to 0 — Sonner's own removal fade
      // doesn't work in this layout, so this is the one chance to animate
      // out before its hardcoded 200ms unmount timer fires
      animate={{ opacity: is_removing ? 0 : STACK_OPACITY[stack_depth] }}
      transition={{ duration: is_removing ? 0.15 : 0.3, ease: "easeOut" }}
      style={{ height: `${containerHeight}px`, zIndex: MAX_STACK_DEPTH - stack_depth }}
      className="flex items-end justify-center w-full pointer-events-none"
      /**
       * Sonner's own <ol> wrapper flips an internal `expanded` state true on
       * hover — a built-in "un-stack to show previews" feature for its own
       * absolute-position stacking system, which this layout doesn't use.
       * The CSS rule it flips on still exists in Sonner's stylesheet though,
       * and computes a bogus offset against custom properties this layout
       * never sets correctly — that's the "toast jumps up sharply on
       * hover" bug. The <ol>'s pointer-events:none doesn't stop this since
       * our content opts back into pointer-events:auto and DOM events still
       * bubble through. Stopping propagation here keeps the hover from ever
       * reaching Sonner's listener.
       */
      onMouseEnter={(e) => e.stopPropagation()}
      onMouseMove={(e) => e.stopPropagation()}
    >
      <motion.div
        layout // auto-animates size/position changes as the pill expands/collapses
        animate={{ scale: is_removing ? 0.9 : STACK_SCALE[stack_depth] }}
        style={{ borderRadius: "999px" }} // stays a pill/circle at any width
        transition={is_removing ? { duration: 0.15, ease: "easeOut" } : springConfig}
        className={cn(
          "relative inline-flex items-center justify-start overflow-hidden pointer-events-auto shrink-0",
          // min-w must match the collapsed icon box's width (w-[36px] below) —
          // was still 44px from before the icon box shrank, leaving a visible gap on the right
          "min-w-[36px] origin-bottom border-2 border-white/90 backdrop-blur-md shadow-[0_6px_20px_rgba(200,160,100,0.25)]",
          backgroundColor,
          className,
        )}
      >
        {/* 1. left icon box */}
        <motion.div
          layout // also tracks layout so it stays in sync with the parent's size change
          className={cn(
            "flex items-center justify-center shrink-0 z-20",
            isExpanded && wasExpanded && hasSub ? "w-[52px] h-[52px]" : "w-[36px] h-[36px]",
          )}
        >
          <div
            className={cn(
              "w-6 h-6 rounded-full bg-[#FFF3DE] flex items-center justify-center text-[#5B4636] transition-transform duration-500",
              wasExpanded && hasSub ? "scale-125" : "scale-100",
            )}
          >
            {icon}
          </div>
        </motion.div>

        {/* 2. variable-width content container */}
        <AnimatePresence>
          {isExpanded && wasExpanded && (
            <motion.div
              layout
              initial={{ opacity: 0, x: -10 }}
              animate={{ opacity: 1, x: 0 }}
              exit={{ opacity: 0, x: -10 }}
              className="flex items-center h-full overflow-hidden shrink-0 whitespace-nowrap pr-4"
            >
              <div className="flex flex-col justify-center py-1">
                <div className="flex items-baseline gap-2 leading-none">
                  <span className="text-[13px] font-semibold text-[#5B4636] tracking-tight">
                    {title}
                  </span>
                  {description && (
                    <span className="text-[9px] font-black text-[#5B4636]/40 uppercase tracking-widest">
                      {description}
                    </span>
                  )}
                </div>
                {subDescription && (
                  <div className="text-[10px] font-medium text-[#5B4636]/50 mt-0.5 italic tracking-tight">
                    {subDescription}
                  </div>
                )}
              </div>
            </motion.div>
          )}
        </AnimatePresence>

        {/* 3. right-side status dot */}
        <AnimatePresence>
          {isExpanded && wasExpanded && statusColor && (
            <motion.div
              layout
              initial={{ opacity: 0, scale: 0 }}
              animate={{ opacity: 1, scale: 1 }}
              exit={{ opacity: 0, scale: 0 }}
              className="flex items-center justify-center pr-4 z-20"
            >
              <div className={cn("w-1.5 h-1.5 rounded-full", statusColor)} />
            </motion.div>
          )}
        </AnimatePresence>
      </motion.div>
    </motion.div>
  );
}
