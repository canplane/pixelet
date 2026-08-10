// @pxl/web/shared/ui/toast/toast.tsx

import { useEffect, useMemo, useRef, useState } from "react";
import type { ReactNode } from "react";
import { animate as animate_value, AnimatePresence, LayoutGroup, motion, useMotionValue } from "framer-motion";
import { cn } from "@/shared/lib/utils/cn.ts";
import { get_toast_stack_rotation, reset_toast_stack_rotation, rotate_toast_stack, set_toast_drag_active, subscribe_toast_stack_rotation } from "./toast_store.ts";

/** How far a stacked-behind toast should peek out below the front one. */
export const TOAST_STACK_PEEK_PX = 6;

/** Starting y (above the resting position) for a card that wraps around
 * from the back of the deck to the front via a downward swipe — see the
 * doc comment above the `just_entered_front_via_down_swipe` computation
 * below for why this needs its own explicit entry point instead of just
 * letting the normal depth-change interpolation handle it. */
const DOWN_SWIPE_ENTRY_Y_PX = -24;

/** How many cards deep the deck visually goes before further-back toasts
 * just stay fully hidden at the last card's own depth (still logically
 * present/counted, just not drawn as yet another visible sliver). */
export const MAX_STACK_DEPTH = 2;

/** Per-depth opacity/scale, front (depth 0) to back (MAX_STACK_DEPTH). */
const STACK_OPACITY = [1, 0.6, 0.3];
const STACK_SCALE = [1, 0.94, 0.88];

/** How far (in px) a swipe on the front card must travel vertically before it rotates the deck one step. */
const STACK_SWIPE_THRESHOLD_PX = 24;

/** Live drag feedback while the pointer is still down — pointerdown/up alone
 * decide the *outcome* fine, but with nothing rendered in between, the pill
 * just sits dead still until release then snaps, which reads as unresponsive
 * (much more noticeable on a slow, deliberate mouse drag than on a quick
 * touch flick). Not a 1:1 follow — this is a rotation cue, not a
 * drag-to-dismiss, so the pill only needs to visibly *give* a little. Same
 * cap both directions — an earlier version let down travel further than up,
 * which read as an unexplained inconsistency rather than a deliberate cue. */
const DRAG_FOLLOW_RATIO = 0.35;
const DRAG_FOLLOW_MAX_PX = 28;

type ToastProps = {
  id: string;
  /** Position among currently-visible toasts, 0 = most recently created — passed straight through from toast_store.ts's container, not derived from anything DOM-observed. */
  index: number;
  /** Total toast count right now, same number for every toast instance in the same render pass. */
  stack_size: number;
  /** Whether toast_store.ts has marked this toast dismissed — still mounted for its own fade-out, but logically gone. */
  is_removing: boolean;
  children?: ReactNode;
  icon?: ReactNode;
  /** The toast's only text content — no separate title/description split, since nothing here ever used more than one line. */
  message: string;
  statusColor?: string;
  className?: string;
  backgroundColor?: string;
};

export function Toast({
  id,
  index,
  stack_size,
  is_removing,
  icon,
  message,
  statusColor,
  className,
  backgroundColor = "bg-[#FFFDF7]/85",
}: ToastProps) {
  const isExpanded = !!message;

  // Plain module-level pub/sub (toast_stack_store.ts), not a React context —
  // every stacked toast is its own top-level render (toast_store.ts's
  // container maps over the list independently for each), so a useState
  // here alone couldn't be seen by the other toast instances that also need
  // to react to the same swipe.
  const [rotation, set_rotation] = useState(get_toast_stack_rotation);
  // Mutated in the subscription callback, not during render — this runs
  // from rotate_toast_stack's own caller (a pointer event handler), well
  // outside React's render phase, so there's no StrictMode double-invoke
  // hazard the way there would be for a ref written directly in the render
  // body. Read below alongside prev_rotated_depth (which *is* derived
  // during render, via the render-safe "adjust state during render"
  // pattern) to catch the one render where this toast becomes the new
  // front card as a direct result of a downward swipe.
  const rotation_direction = useRef<-1 | 0 | 1>(0);
  useEffect(
    () =>
      subscribe_toast_stack_rotation((r, direction) => {
        rotation_direction.current = direction;
        set_rotation(r);
      }),
    [],
  );

  // nothing left to rotate (or this toast itself is on its way out) — reset
  // so a lone remaining toast never sits at a stale rotated depth, and so
  // the next time a second toast shows up the deck starts unrotated instead
  // of inheriting whatever offset was left over from before.
  useEffect(() => {
    if (stack_size <= 1 && rotation !== 0) reset_toast_stack_rotation();
  }, [stack_size, rotation]);

  /**
   * Multiple toast ids can legitimately be up at once — this reads as a
   * card deck: the newest takes the one fixed front slot, and the previous
   * occupant slides back a depth, fading/shrinking more per level. A swipe
   * on the front card rotates the deck (true circular queue: rotation
   * wraps mod stack_size, so enough swipes cycle the front card all the
   * way back around instead of just marching depth off toward infinity).
   *
   * Deliberately UNCLAMPED — a toast queued past MAX_STACK_DEPTH still gets
   * a real, continuous depth number (3, 4, 5, ...) instead of being pinned
   * to MAX_STACK_DEPTH or unmounted. is_visible below just forces its
   * opacity to 0 at that point; the element itself, its position, and its
   * mount identity never change because of visibility. That's the actual
   * fix for every "toast moves on its own" report so far: all of them
   * traced back to something re-mounting or re-anchoring a toast's
   * `initial` state (this component returning null and remounting later,
   * or a one-off "entered from a different position" override) — a fresh
   * mount, not a re-render, is what let framer-motion treat some new
   * position as the animation's starting point. A component that never
   * unmounts and whose `initial` and `animate` always describe the exact
   * same resting point when nothing has changed literally cannot move on
   * its own: framer-motion only ever animates a value that actually
   * changed between renders, full stop, regardless of how many unrelated
   * things elsewhere caused this render to happen.
   */
  const rotated_depth = stack_size > 0 ? ((index - rotation) % stack_size + stack_size) % stack_size : 0;

  /**
   * Detects the one render where this toast becomes the new front card as
   * a direct result of a downward swipe — the card that wraps around from
   * the back of the deck. Every other depth change (including the
   * up-swipe's own "front card wraps to the back, next one steps forward"
   * case) already reads fine just interpolating stack_y/opacity normally,
   * since deeper depths are always positioned *below* the front slot —
   * moving toward depth 0 is always an upward motion regardless of which
   * way the deck was swiped. A downward swipe pulling the back card to the
   * front is the one case where that upward-only motion fights the
   * gesture's own direction, so this one transition gets an explicit
   * "descend from above, fading in" entrance instead (see
   * DOWN_SWIPE_ENTRY_Y_PX below).
   *
   * A ref updated from `useEffect` (post-commit), not during render — an
   * earlier version used React's "adjust state during render" idiom
   * (calling a state setter conditionally in the render body) to dodge a
   * StrictMode double-invoke hazard, but that idiom is only safe when the
   * derived boolean *isn't* also read in the same render that adjusts the
   * state: calling the setter during render makes React discard that
   * render and immediately re-render with the state already updated — so
   * `prev_rotated_depth` had already caught up to `rotated_depth` by the
   * time the *actually-committed* render read it back, and this was
   * always false in practice. Updating the ref from an effect instead
   * means the comparison during render always sees last commit's value,
   * never this one's — reads correctly, and (since effects don't
   * re-run per StrictMode's render-only double-invoke) doesn't reintroduce
   * the double-invoke problem a plain ref-write-during-render would have
   * had. Self-resetting: once this fires, the effect below updates the ref
   * to match, so the very next render (even an unrelated one) no longer
   * satisfies the condition — doesn't fight the "never animates without
   * something actually changing" invariant above.
   */
  const prev_rotated_depth = useRef(rotated_depth);
  const just_entered_front_via_down_swipe = prev_rotated_depth.current !== 0 && rotated_depth === 0 && rotation_direction.current === -1;
  useEffect(() => {
    prev_rotated_depth.current = rotated_depth;
  }, [rotated_depth]);

  const is_visible = rotated_depth <= MAX_STACK_DEPTH;
  // clamped only for table lookups (STACK_OPACITY/STACK_SCALE, z-index) —
  // never used to decide position or mount identity.
  const stack_depth = Math.min(rotated_depth, MAX_STACK_DEPTH);

  // tuned specifically for the pill's own width growth (icon -> expanded
  // text) via its `layout` prop below — framer-motion's default layout
  // spring is untuned for that and visibly lagged/overshot against the
  // wrapper's own (differently-timed) opacity/y/scale transition, so the
  // two no longer moved in lockstep. is_removing still needs the same fast
  // override the wrapper uses, or the pill's width collapses on its own
  // slower default schedule while the wrapper's already faded out.
  const springConfig = useMemo(
    () => ({
      type: "spring" as const,
      stiffness: 500,
      // Critical damping for stiffness 500 / mass 0.5 is 2*sqrt(500*0.5) ≈
      // 31.6 — 30 sat just under that, i.e. technically underdamped, so
      // ANY layout re-measurement (including a spurious re-render whose
      // target didn't actually move, e.g. sub-pixel rounding noise) would
      // visibly ring/overshoot before settling instead of just... not
      // moving. That's what a "for no reason" twitch on an already-settled
      // toast actually was: not a bug in when this fires, a bug in how it
      // resolves once triggered. 40 is comfortably over critical — even a
      // spurious re-trigger now settles with zero oscillation.
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

  // depth 0 (the front card) is the only one the user can actually grab —
  // deeper cards mostly sit hidden behind it in the same screen area, so
  // only it carries the drag gesture. Nothing to rotate (and thus nothing
  // to drag) with just one toast up, and a card already animating out
  // shouldn't newly accept input.
  const is_draggable = stack_depth === 0 && stack_size > 1 && !is_removing;

  const stack_y = is_removing ? 0 : stack_depth * TOAST_STACK_PEEK_PX;
  // is_visible forces opacity to 0 for anything past MAX_STACK_DEPTH — the
  // clamped stack_depth alone would give it STACK_OPACITY[MAX_STACK_DEPTH]
  // (dim, not invisible), sitting exactly coincident with the actual last
  // visible card. Position/scale don't need their own is_visible branch:
  // they're already coincident with that same card, so there's nothing to
  // additionally hide there.
  const stack_opacity = is_removing ? 0 : (is_visible ? STACK_OPACITY[stack_depth] : 0);
  const stack_scale = is_removing ? 0.9 : STACK_SCALE[stack_depth];

  /**
   * The wrapper's own y/opacity, driven imperatively (same idiom as drag_y
   * further below) instead of through framer-motion's declarative `animate`
   * prop — an `animate={{ y: [from, to] }}` keyframe array was tried first
   * for the down-swipe entrance and didn't actually override the "continue
   * from wherever the value is currently settled" default the way the docs
   * suggest it should; the card still visibly rose up from its old
   * (below-front) position instead of dropping in from
   * DOWN_SWIPE_ENTRY_Y_PX. Explicitly `.set()`-ing a hard jump before
   * calling `animate_value` sidesteps that ambiguity entirely: the starting
   * point is never in question because it's set directly, not inferred.
   *
   * Only y/opacity move here — scale has no directional entrance to get
   * right, so it stays on the plain declarative `animate` prop below,
   * unaffected by any of this.
   */
  const wrapper_y = useMotionValue(stack_y);
  const wrapper_opacity = useMotionValue(0);

  useEffect(() => {
    if (just_entered_front_via_down_swipe) {
      wrapper_y.set(DOWN_SWIPE_ENTRY_Y_PX);
      wrapper_opacity.set(0);
    }
    const wrapper_transition = { duration: is_removing ? 0.15 : 0.3, ease: "easeOut" as const };
    animate_value(wrapper_y, stack_y, wrapper_transition);
    animate_value(wrapper_opacity, stack_opacity, wrapper_transition);
    // wrapper_y/wrapper_opacity are stable MotionValue refs (framer-motion
    // guarantees identity across renders, same as useRef) — deliberately
    // excluded so eslint doesn't ask for them.
    // deno-lint-ignore react-hooks/exhaustive-deps
  }, [stack_y, stack_opacity, just_entered_front_via_down_swipe, is_removing]);

  /**
   * Swipe-to-rotate via raw pointer events, not framer-motion's `drag`.
   * The actual "only the first swipe ever worked" bug turned out to be
   * toaster.tsx's per-toast wrapper (see its own doc comment): a stray
   * `transform` there created a new stacking context that trapped this
   * component's z-index inside just that one wrapper, so sibling toasts
   * ended up stacked by DOM order instead — pointerdown/up landed on
   * whichever toast was DOM-last (usually the oldest), not the one that
   * visually looked frontmost. Plain pointer events over `drag` is kept
   * anyway on general principle (see the global "orthogonal, not
   * composite" note in CLAUDE.md): gesture detection here is two DOM
   * listeners and a Y delta with zero dependency on an animation
   * library's own internal state.
   *
   * move/up/cancel are bound to `window`, not `setPointerCapture` on the
   * pill — matching the pattern camera/input.tsx already uses for its own
   * mouse drag (down on the element, up/cancel on window). Capture ties
   * delivery of every subsequent event for that pointerId to one specific
   * DOM node staying valid and un-replaced for the gesture's whole
   * duration; a touch device's own implicit capture papers over any hiccup
   * there, but a real mouse has no such fallback — it depends entirely on
   * our explicit setPointerCapture call succeeding and holding, on a node
   * that this same component is also independently animating every frame
   * (layout spring + the drag_y motion value below). Listening on window
   * sidesteps that dependency completely: it sees the pointerId's events
   * regardless of which node they'd otherwise target.
   */
  const drag_start_y = useRef<number | null>(null);
  const active_pointer_id = useRef<number | null>(null);
  // Rotation now fires the instant the drag crosses the threshold — not on
  // release — so this guards the rest of THAT SAME drag from firing again:
  // without it, holding past the threshold (or wobbling back and forth
  // across it) would rotate the deck once per pixel/move event instead of
  // once per gesture. Set false on pointerdown, true the moment a rotate
  // fires, never re-armed until the next pointerdown starts a new drag.
  const has_rotated_this_drag = useRef(false);

  // Raw MotionValue, not React state — pointermove can fire far more often
  // than a React render is worth paying for, and framer-motion writes a
  // MotionValue straight to the DOM style without going through React at
  // all. Still entirely our own gesture math (see doc comment above); this
  // is just the output pipe, not a framer-motion gesture/drag mechanism.
  const drag_y = useMotionValue(0);

  function end_drag() {
    animate_value(drag_y, 0, springConfig);
  }

  function handle_window_pointer_move(e: PointerEvent) {
    if (drag_start_y.current === null || e.pointerId !== active_pointer_id.current) return;
    if (has_rotated_this_drag.current) return; // already committed this drag — see below

    const raw_delta = e.clientY - drag_start_y.current;

    // Rotation always demotes the dragged card by exactly one depth step,
    // which always pushes its stack_y *positive* (down) regardless of
    // which way you swiped. Left drag_y following the pointer until
    // release, that combined additively with a down-swipe's own already-
    // positive drag_y (both pushing further down at once — looked
    // unfinished until release snapped it back) but partially canceled an
    // up-swipe's negative drag_y (looked instantly settled). Ending the
    // follow the moment rotation actually fires — instead of waiting for
    // pointerup — makes both directions resolve the same way: a single
    // clean transition into the new depth, no direction-dependent
    // leftover offset to untangle.
    if (raw_delta < -STACK_SWIPE_THRESHOLD_PX) {
      has_rotated_this_drag.current = true;
      end_drag();
      rotate_toast_stack(1);
      return;
    }
    if (raw_delta > STACK_SWIPE_THRESHOLD_PX) {
      has_rotated_this_drag.current = true;
      end_drag();
      rotate_toast_stack(-1);
      return;
    }

    const followed = Math.max(-DRAG_FOLLOW_MAX_PX, Math.min(DRAG_FOLLOW_MAX_PX, raw_delta * DRAG_FOLLOW_RATIO));
    drag_y.set(followed);
  }

  function detach_window_listeners() {
    window.removeEventListener("pointermove", handle_window_pointer_move);
    window.removeEventListener("pointerup", finish_drag);
    window.removeEventListener("pointercancel", finish_drag);
  }

  // Shared by both pointerup and pointercancel — rotation itself already
  // happened (if it was going to) back in handle_window_pointer_move, so
  // release just ends the live-follow feedback and tears the gesture down.
  function finish_drag(e: PointerEvent) {
    if (drag_start_y.current === null || e.pointerId !== active_pointer_id.current) return;
    drag_start_y.current = null;
    active_pointer_id.current = null;
    detach_window_listeners();
    end_drag();
    // Applies any background-toast promotion that arrived mid-drag and got
    // deferred — see set_toast_drag_active's own doc comment in
    // toast_store.ts.
    set_toast_drag_active(false);
  }

  function handle_pointer_down(e: React.PointerEvent<HTMLDivElement>) {
    if (!is_draggable) return;
    drag_start_y.current = e.clientY;
    active_pointer_id.current = e.pointerId;
    has_rotated_this_drag.current = false;
    window.addEventListener("pointermove", handle_window_pointer_move);
    window.addEventListener("pointerup", finish_drag);
    window.addEventListener("pointercancel", finish_drag);
    set_toast_drag_active(true);
  }

  // safety net if the component unmounts mid-drag (e.g. dismissed from
  // elsewhere while the user's still holding it) — window listeners
  // otherwise outlive the component and keep firing into stale closures,
  // and set_toast_drag_active(false) (harmless no-op if no drag was
  // actually active) keeps the store from getting stuck thinking a drag is
  // still in progress forever, which would silently swallow every
  // background-toast promotion from then on.
  useEffect(() => () => {
    detach_window_listeners();
    set_toast_drag_active(false);
  }, []);

  return (
    // Own LayoutGroup per toast id — framer-motion otherwise coordinates
    // every `layout`-tracked element on the page (pill, icon box, text
    // reveal — three of them per toast) through ONE shared projection
    // tree by default. An actively-animating layout element in ONE toast
    // (e.g. the orientation-status pill resizing as its text changes on
    // the sensor watchdog's own schedule) could bump/re-project a
    // completely unrelated, already-settled toast's layout right along
    // with it — a real re-measurement, not a React re-render, so neither
    // the memo boundary in toaster.tsx nor anything about this component's
    // own render triggers explains it. Scoping each toast to its own group
    // keeps that projection work from ever crossing between toasts.
    <LayoutGroup id={id}>
      {
        /**
         * wrapper: the box size the toast container (toaster.tsx) sizes its own
         * per-toast anchor around. Owns every depth-driven visual value —
         * opacity, y-offset, scale, z-index — all in one place, on an element
         * with no gesture and no content-driven size changes of its own. The
         * pill below is the opposite: it owns gesture detection and its own
         * content-driven WIDTH growth (icon -> text reveal), nothing
         * depth-related — see its own doc comment.
         *
         * y/opacity are wrapper_y/wrapper_opacity (imperative MotionValues,
         * see their own doc comment above) rather than framer's declarative
         * `animate` — scale stays declarative below since it has no
         * directional-entrance case to get right.
         *
         * A version of this DID look broken once (every depth rendering with
         * near-identical opacity), which briefly looked like the depth math
         * itself not differentiating. The real cause was toaster.tsx's
         * per-toast wrapper: a stray `transform` there created a new stacking
         * context that trapped this element's z-index inside just that one
         * wrapper, so siblings painted by DOM order instead of by z-index —
         * a dim, DOM-last toast could paint over a bright, z-index-front one,
         * which reads exactly like "opacity isn't differentiating." That's
         * fixed at the source now (see toaster.tsx).
         *
         * The y here is the whole card-deck "peek": every toast anchors to the
         * exact same fixed point (toaster.tsx's container), so without this a
         * demoted toast would sit fully behind the front one with no sliver
         * showing. It's a small offset purely a function of this toast's own
         * (rotated) stack_depth — never derived from another toast's measured
         * size or DOM position.
         *
         * `initial` pins scale to this exact same render's own target — it
         * never actually animates at mount either, same reasoning as y used
         * to have before it moved to wrapper_y (see that comment): there is
         * no code path where initial and target scale differ, which is part
         * of what guarantees a toast never moves except in response to its
         * own stack_depth genuinely changing after mount.
         */
      }
      <motion.div
        initial={{ scale: stack_scale }}
        animate={{ scale: stack_scale }}
        transition={{ duration: is_removing ? 0.15 : 0.3, ease: "easeOut" }}
        style={{
          position: "relative",
          // z-index is a no-op on a statically-positioned element (the CSS
          // default), so it's set explicitly — otherwise every toast paints
          // in DOM order regardless of stack_depth, no matter how "correct"
          // zIndex's own number is.
          zIndex: MAX_STACK_DEPTH - stack_depth,
          transformOrigin: "center bottom",
          y: wrapper_y,
          opacity: wrapper_opacity,
        }}
        className="flex items-end justify-center w-full h-8 pointer-events-none"
      >
        {
          /**
           * Expands the swipe hit area a bit past the pill's own visible
           * edges — negative margin + equal padding is the standard
           * "bigger hit target, zero visual/layout footprint" trick: the
           * margin pulls this box's contribution to the flex row back to
           * exactly the pill's real size, while the padding pushes the
           * actual interactive box back out by the same amount, so the pill
           * renders identically but pointerdown now registers up to 8px
           * outside its rounded edge. Lives here (not on the pill itself)
           * because the pill has overflow-hidden (for clipping its own
           * resizing content) — that clips any pseudo-element/padding trick
           * placed on the pill directly, so the hit area needs its own
           * unclipped wrapper.
           *
           * is_draggable-gated (margin/padding both, not just the handler) —
           * only the front card is ever interactive; stacked-behind cards'
           * own handler already no-ops via handle_pointer_down's own guard,
           * but leaving their hit box expanded too was pure downside: it'd
           * reach past TOAST_STACK_PEEK_PX (6px, the peek offset between
           * depths) into the next card's own peeking edge for no functional
           * benefit.
           */
        }
        <div
          onPointerDown={handle_pointer_down}
          style={is_draggable ? { margin: "-8px", padding: "8px" } : undefined}
          className={cn("inline-flex", is_visible ? "pointer-events-auto" : "pointer-events-none", is_draggable && "touch-none")}
        >
          <motion.div
            layout
            transition={is_removing ? { duration: 0.15, ease: "easeOut" } : springConfig}
            style={{ borderRadius: "999px", y: drag_y }}
            /**
             * motion.div, not plain div — matches the wrapper above and gets
             * framer-motion's own pointer-event plumbing (correct touch-action
             * handling, passive-listener setup) under the hand-rolled gesture
             * below, instead of a bare DOM node. `layout` here is separate from
             * that: it's so the pill's own width (icon-only vs. expanded text)
             * tweens smoothly, same as its motion.div children already do —
             * unrelated to the gesture, no `drag` prop involved.
             *
             * Swipe-to-rotate the stack: only the front card (is_draggable) is
             * grabbable — see its own comment above. Plain pointerdown/pointerup
             * (not framer-motion's `drag`, see the doc comment on
             * handle_pointer_down/up above) — pointerup reads the total Y
             * travel and decides whether to rotate the deck; the actual card-
             * swap comes from every toast's own rotated stack_depth reacting to
             * the shared rotation value, not from anything this element's own
             * position does.
             *
             * Swipe up (negative y) sends the front card to the back and steps
             * the next one forward; swipe down reverses one step — full
             * circular queue, so swiping forward stack_size times cycles all
             * the way back to today's front card. onPointerDown lives on the
             * hit-area wrapper above, not here — see its own doc comment.
             */
            className={cn(
              "relative inline-flex items-center justify-start overflow-hidden shrink-0",
              // h-8 explicitly on the pill itself, not left implicit — the
              // pill's own height used to just be whatever its tallest child
              // (the icon box, always present) happened to be. Once the icon
              // box became conditional (no icon -> not rendered at all), a
              // toast with no icon had nothing establishing a height during
              // the collapsed window before wasExpanded flips, so the pill
              // collapsed to ~0 then snapped open — and the text container's
              // own `h-full` (below) had nothing to resolve against either,
              // breaking its vertical centering. Fixing the height here makes
              // both independent of whether an icon happens to be passed.
              // min-w matches that same h-8 so the collapsed pill (icon or
              // not) reads as a circle, not a squashed oval.
              "h-8 min-w-8 origin-bottom border-2 border-white/90 backdrop-blur-md shadow-[0_6px_20px_rgba(200,160,100,0.25)]",
              // select-none: a status toast's text was never meant to be
              // copied, and without this a swipe-to-rotate drag (or even just a
              // slightly-off-axis mouse drag on desktop) fights the browser's
              // native text-selection drag on the same gesture.
              "select-none",
              backgroundColor,
              className,
            )}
          >
            {
              /* 1. left icon slot — fixed size, doesn't vary with content. Not
        rendered at all without an icon — an empty box isn't a placeholder
        worth showing. No background/circle of its own (removed — barely
        distinguishable from the pill's own near-white background in the
        common case, only actually read as a badge against the red error
        background, not worth the layer for that alone); the icon renders
        directly, its own color/stroke carries the visual weight. */
            }
            {icon && (
              <motion.div
                layout // also tracks layout so it stays in sync with the parent's size change
                // Same spring as the pill itself (springConfig) — without this,
                // `layout` alone falls back to framer-motion's own default
                // spring, which doesn't match the pill's tuned one, so this box
                // and the pill's own edge visibly moved on two different
                // curves instead of in lockstep.
                transition={is_removing ? { duration: 0.15, ease: "easeOut" } : springConfig}
                // -mx-0.5: offsets the pill's own border-2 (2px each side) so
                // this box's intrinsic width doesn't push the pill past its
                // min-w-8 floor — border-box makes h-8 clip to exactly 32px
                // regardless of content, but min-w-8 is only a floor, so
                // without this the pill's auto-width (this box's 32px +
                // border's 4px) settled at 36px, an oval instead of a circle
                // in the collapsed (icon-only) state.
                className="flex items-center justify-center shrink-0 z-20 w-8 h-8 -mx-0.5"
              >
                {icon}
              </motion.div>
            )}

            {
              /* 2. variable-width content container — padding is per-side, not a
        flat px, because each side's job differs: when an icon/dot slot
        sits next to it, that slot already clears the pill's rounded cap
        (see its own w-8 h-8 box), so text only needs a small gap from it.
        With no neighbor there (icon-less/dot-less toasts, e.g. dummy
        toasts) text is the only thing on that side and has to clear the
        cap on its own, so it needs the bigger inset instead. */
            }
            <AnimatePresence>
              {isExpanded && wasExpanded && (
                <motion.div
                  layout
                  // same spring as the pill and icon box — see the icon box's
                  // own comment on why this can't be left implicit.
                  transition={is_removing ? { duration: 0.15, ease: "easeOut" } : springConfig}
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

            {
              /* 3. right-side status dot — same w-8 h-8 slot as the left
        icon box (not a bare dot floating in pr-4 padding), so its box is
        exactly as tall as the pill and sits flush at the pill's own right
        edge: concentric with the right rounded cap, the same way the icon
        box is concentric with the left one. Box width, box margin
        (-mx-0.5), and the text's own padding (pl-1/pr-1) are all identical
        on both sides — the box-to-text gap this actually controls is
        symmetric by construction, regardless of how much smaller the dot
        itself looks than the icon inside its equally-sized box. */
            }
            <AnimatePresence>
              {isExpanded && wasExpanded && statusColor && (
                <motion.div
                  layout
                  // same spring as the pill/icon box/text — see the icon box's
                  // own comment.
                  transition={is_removing ? { duration: 0.15, ease: "easeOut" } : springConfig}
                  initial={{ opacity: 0, scale: 0 }}
                  animate={{ opacity: 1, scale: 1 }}
                  exit={{ opacity: 0, scale: 0 }}
                  // -mx-0.5: same border-offset fix as the icon box above.
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
