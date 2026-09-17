// @pxl/web/kit/ui/toast/toast.tsx

import { useEffect, useMemo, useRef, useState } from "react";
import type { ReactNode } from "react";
import { animate as animateValue, AnimatePresence, LayoutGroup, motion, useMotionValue } from "framer-motion";
import { cn } from "@/kit/lib/util/cn.ts";
import { getToastStackRotation, resetToastStackRotation, rotateToastStack, setToastDragActive, subscribeToastStackRotation } from "./store.ts";

export const TOAST_STACK_PEEK_PX = 6;

const DOWN_SWIPE_ENTRY_Y_PX = -24;

export const MAX_STACK_DEPTH = 2;

const STACK_OPACITY = [1, 0.6, 0.3];
const STACK_SCALE = [1, 0.94, 0.88];

const STACK_SWIPE_THRESHOLD_PX = 24;

const DRAG_FOLLOW_RATIO = 0.35;
const DRAG_FOLLOW_MAX_PX = 28;

type ToastProps = {
  id: string;
  index: number;
  stackSize: number;
  isRemoving: boolean;
  children?: ReactNode;
  icon?: ReactNode;
  message: string;
  statusColor?: string;
  className?: string;
  backgroundColor?: string;
};

export function Toast({
  id,
  index,
  stackSize,
  isRemoving,
  icon,
  message,
  statusColor,
  className,
  backgroundColor = "bg-[#FFFDF7]",
}: ToastProps) {
  const isExpanded = !!message;

  const [rotation, setRotation] = useState(getToastStackRotation);
  // Written from the subscription callback (a pointer-event handler), not
  // during render — so unlike a ref write during render, there's no
  // StrictMode double-invoke hazard here.
  const rotationDirection = useRef<-1 | 0 | 1>(0);
  useEffect(
    () =>
      subscribeToastStackRotation((r, direction) => {
        rotationDirection.current = direction;
        setRotation(r);
      }),
    [],
  );

  useEffect(() => {
    if (stackSize <= 1 && rotation !== 0) resetToastStackRotation();
  }, [stackSize, rotation]);

  // Deliberately unclamped — a toast queued past MAX_STACK_DEPTH still gets
  // a real, continuous depth (3, 4, 5, ...) instead of being pinned or
  // unmounted; isVisible below just forces its opacity to 0. Clamping the
  // depth itself would let visibility alone change this toast's position,
  // which breaks the "never moves except when stackDepth genuinely
  // changes" invariant the rest of this component relies on.
  const rotatedDepth = stackSize > 0 ? ((index - rotation) % stackSize + stackSize) % stackSize : 0;

  // Detects the render where this card becomes the new front via a
  // downward swipe (wraps from the back of the deck) — every other depth
  // change is an upward motion toward depth 0, but this one case needs an
  // explicit "descend from above" entrance instead (DOWN_SWIPE_ENTRY_Y_PX).
  //
  // Updated from useEffect (post-commit), not during render: writing the
  // ref during render and reading it back in the same render collapses via
  // React's state-adjustment-during-render idiom, so the comparison would
  // always be false. Reading from effect means it always sees last
  // commit's value.
  const prevRotatedDepth = useRef(rotatedDepth);
  const justEnteredFrontViaDownSwipe = prevRotatedDepth.current !== 0 && rotatedDepth === 0 && rotationDirection.current === -1;
  useEffect(() => {
    prevRotatedDepth.current = rotatedDepth;
  }, [rotatedDepth]);

  const isVisible = rotatedDepth <= MAX_STACK_DEPTH;
  const stackDepth = Math.min(rotatedDepth, MAX_STACK_DEPTH);

  const springConfig = useMemo(
    () => ({
      type: "spring" as const,
      stiffness: 500,
      // Critical damping for stiffness 500 / mass 0.5 is 2*sqrt(500*0.5) ≈
      // 31.6 — 30 sat just under that (underdamped), so any re-measurement
      // (even a spurious one whose target didn't move) would visibly
      // ring/overshoot before settling. 40 is comfortably over critical.
      damping: 40,
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

  const isDraggable = stackDepth === 0 && stackSize > 1 && !isRemoving;

  const stackY = isRemoving ? 0 : stackDepth * TOAST_STACK_PEEK_PX;
  const stackOpacity = isRemoving ? 0 : (isVisible ? STACK_OPACITY[stackDepth] : 0);
  const stackScale = isRemoving ? 0.9 : STACK_SCALE[stackDepth];

  // Driven imperatively (like dragY below) rather than through framer's
  // declarative `animate` — an `animate={{ y: [from, to] }}` keyframe array
  // didn't reliably override the "continue from wherever the value
  // currently is" default for the down-swipe entrance. Explicitly `.set()`
  // to a hard jump before calling `animateValue` removes the ambiguity.
  const wrapperY = useMotionValue(stackY);
  const wrapperOpacity = useMotionValue(0);

  useEffect(() => {
    if (justEnteredFrontViaDownSwipe) {
      wrapperY.set(DOWN_SWIPE_ENTRY_Y_PX);
      wrapperOpacity.set(0);
    }
    const wrapperTransition = { duration: isRemoving ? 0.15 : 0.3, ease: "easeOut" as const };
    animateValue(wrapperY, stackY, wrapperTransition);
    animateValue(wrapperOpacity, stackOpacity, wrapperTransition);
    // wrapperY/wrapperOpacity are stable MotionValue refs (framer-motion
    // guarantees identity across renders, same as useRef) — deliberately
    // excluded from the deps below.
  }, [stackY, stackOpacity, justEnteredFrontViaDownSwipe, isRemoving]);

  const dragStartY = useRef<number | null>(null);
  const activePointerId = useRef<number | null>(null);
  // Rotation fires the instant the drag crosses the threshold, not on
  // release — this guards the rest of the same drag from firing again, so
  // holding past the threshold rotates the deck once per gesture, not once
  // per pixel.
  const hasRotatedThisDrag = useRef(false);

  const dragY = useMotionValue(0);

  function endDrag() {
    animateValue(dragY, 0, springConfig);
  }

  function handleWindowPointerMove(e: PointerEvent) {
    if (dragStartY.current === null || e.pointerId !== activePointerId.current) return;
    if (hasRotatedThisDrag.current) return;

    const rawDelta = e.clientY - dragStartY.current;

    // Ending the follow the moment rotation fires (rather than waiting for
    // pointerup) makes both swipe directions resolve the same way: rotation
    // always demotes the card, which always pushes dragY positive — left
    // to keep following the pointer until release, that would additively
    // stack with a down-swipe's own positive delta, or partially cancel an
    // up-swipe's negative one.
    if (rawDelta < -STACK_SWIPE_THRESHOLD_PX) {
      hasRotatedThisDrag.current = true;
      endDrag();
      rotateToastStack(1);
      return;
    }
    if (rawDelta > STACK_SWIPE_THRESHOLD_PX) {
      hasRotatedThisDrag.current = true;
      endDrag();
      rotateToastStack(-1);
      return;
    }

    const followed = Math.max(-DRAG_FOLLOW_MAX_PX, Math.min(DRAG_FOLLOW_MAX_PX, rawDelta * DRAG_FOLLOW_RATIO));
    dragY.set(followed);
  }

  function detachWindowListeners() {
    window.removeEventListener("pointermove", handleWindowPointerMove);
    window.removeEventListener("pointerup", finishDrag);
    window.removeEventListener("pointercancel", finishDrag);
  }

  function finishDrag(e: PointerEvent) {
    if (dragStartY.current === null || e.pointerId !== activePointerId.current) return;
    dragStartY.current = null;
    activePointerId.current = null;
    detachWindowListeners();
    endDrag();
    setToastDragActive(false);
  }

  function handlePointerDown(e: React.PointerEvent<HTMLDivElement>) {
    if (!isDraggable) return;
    dragStartY.current = e.clientY;
    activePointerId.current = e.pointerId;
    hasRotatedThisDrag.current = false;
    window.addEventListener("pointermove", handleWindowPointerMove);
    window.addEventListener("pointerup", finishDrag);
    window.addEventListener("pointercancel", finishDrag);
    setToastDragActive(true);
  }

  // Safety net if the component unmounts mid-drag: window listeners
  // otherwise outlive it and fire into stale closures.
  useEffect(() => () => {
    detachWindowListeners();
    setToastDragActive(false);
  }, []);

  return (
    // Own LayoutGroup per toast id — framer-motion otherwise coordinates
    // every `layout`-tracked element on the page through one shared
    // projection tree, so an actively-animating layout element in one toast
    // could re-project an unrelated, already-settled toast right along with
    // it. Scoping each toast to its own group keeps that from crossing.
    <LayoutGroup id={id}>
      <motion.div
        initial={{ scale: stackScale }}
        animate={{ scale: stackScale }}
        transition={{ duration: isRemoving ? 0.15 : 0.3, ease: "easeOut" }}
        style={{
          position: "relative",
          // zIndex is a no-op on a statically-positioned element by CSS
          // default, so it has to be set explicitly here.
          zIndex: MAX_STACK_DEPTH - stackDepth,
          transformOrigin: "center bottom",
          y: wrapperY,
          opacity: wrapperOpacity,
        }}
        className="flex items-end justify-center w-full h-8 pointer-events-none"
      >
        <div
          onPointerDown={handlePointerDown}
          // Negative margin + equal padding expands the swipe hit area past
          // the pill's own edges with zero visual footprint. Kept on this
          // wrapper rather than the pill itself, since the pill's own
          // overflow-hidden (for clipping its resizing content) would clip
          // the trick.
          style={isDraggable ? { margin: "-8px", padding: "8px" } : undefined}
          className={cn("inline-flex", isVisible ? "pointer-events-auto" : "pointer-events-none", isDraggable && "touch-none")}
        >
          <motion.div
            layout
            transition={isRemoving ? { duration: 0.15, ease: "easeOut" } : springConfig}
            style={{ borderRadius: "999px", y: dragY }}
            className={cn(
              "relative inline-flex items-center justify-start overflow-hidden shrink-0",
              // Explicit h-8, not left implicit: without an icon present,
              // nothing else establishes the pill's height, so it would
              // collapse to ~0 and then snap open once wasExpanded flips.
              // min-w matches so the collapsed pill reads as a circle.
              "h-8 min-w-8 origin-bottom border-2 border-white/90 shadow-[0_6px_20px_rgba(0,0,0,0.25)]",
              // Without this, a swipe-to-rotate drag fights the browser's
              // native text-selection drag on the same gesture.
              "select-none",
              backgroundColor,
              className,
            )}
          >
            {icon && (
              <motion.div
                layout
                // Same spring as the pill itself — otherwise `layout` alone
                // falls back to framer-motion's default spring, and this box
                // visibly desyncs from the pill's own tuned curve.
                transition={isRemoving ? { duration: 0.15, ease: "easeOut" } : springConfig}
                // -mx-0.5 offsets the pill's own border-2 so this box's
                // intrinsic width doesn't push the pill past min-w-8 into an
                // oval instead of a circle in the collapsed state.
                className="flex items-center justify-center shrink-0 z-20 w-8 h-8 -mx-0.5"
              >
                {icon}
              </motion.div>
            )}

            <AnimatePresence>
              {isExpanded && wasExpanded && (
                <motion.div
                  layout
                  transition={isRemoving ? { duration: 0.15, ease: "easeOut" } : springConfig}
                  initial={{ opacity: 0, x: -10 }}
                  animate={{ opacity: 1, x: 0 }}
                  exit={{ opacity: 0, x: -10 }}
                  className={cn(
                    "flex items-center h-full overflow-hidden shrink-0 whitespace-nowrap",
                    icon ? "pl-1" : "pl-3",
                    statusColor ? "pr-1" : "pr-3",
                  )}
                >
                  <span className="text-[13px] font-semibold text-[#5B4636] tracking-tight leading-none">
                    {message}
                  </span>
                </motion.div>
              )}
            </AnimatePresence>

            <AnimatePresence>
              {isExpanded && wasExpanded && statusColor && (
                <motion.div
                  layout
                  transition={isRemoving ? { duration: 0.15, ease: "easeOut" } : springConfig}
                  initial={{ opacity: 0, scale: 0 }}
                  animate={{ opacity: 1, scale: 1 }}
                  exit={{ opacity: 0, scale: 0 }}
                  className="flex items-center justify-center shrink-0 z-20 w-8 h-8 -mx-0.5"
                >
                  <div className={cn("w-1.5 h-1.5 rounded-full", statusColor)} />
                </motion.div>
              )}
            </AnimatePresence>
          </motion.div>
        </div>
      </motion.div>
    </LayoutGroup>
  );
}
