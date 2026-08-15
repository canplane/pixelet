// @pxl/web/modules/map/ui/overlay/mode_bar.tsx

import { useRef, useState } from "react";
import { createPortal } from "react-dom";
import { AnimatePresence, LayoutGroup, motion } from "framer-motion";
import { Eraser, MessageCircle, MousePointer, Palette, Send } from "lucide-react";

import type { f64 } from "@xpute/core/kit/word.ts";
import { cn } from "@/shared/lib/utils/cn.ts";
import { useUiStore } from "@/app/store/ui_store.ts";
import { object_color_hex, object_color_ids, object_scale_exponents, type ObjectColorId } from "@pxl/shared/spec/cell/objects.ts";
import { OBJECT_SIZES } from "@/modules/world/objects/size.ts";

type ModeId = "message" | "palette";

const SPRING = { type: "spring" as const, stiffness: 500, damping: 35, mass: 0.6 };

// How long the input's fade-out plays before send_message actually clears
// message_text — matches the input's own transition-opacity duration below,
// so the text disappears exactly as the fade finishes, not before/after it.
const SEND_FADE_MS = 200;

// Tailwind classes per OBJECT_SIZES tier — static strings (not a computed
// w-[Npx]) so the JIT scanner can actually see them. Add another step if a
// 5th tier ever gets added. Shared by the size-button styling below.
// Halves each step (24/16/8/4px) rather than the previous shallow 16/12/10/8
// taper — with only 3 tiers actually shown (see SIZE_TIERS_TOP_TO_BOTTOM's
// own 1/8u filter), the steps need to read as clearly different at a glance,
// not just technically different. Still comfortably inside the w-9 h-9
// (36px) hit target at the top end.
function size_button_dims(i: number): string {
  return i === 0 ? "w-6 h-6" : i === 1 ? "w-4 h-4" : i === 2 ? "w-2 h-2" : "w-1 h-1";
}

// Per-swatch flyout tiers, top-to-bottom: smallest first, largest last
// (nearest the button that opened it) — reverse of OBJECT_SIZES' own
// largest-first order. item_refs/drag_index/on_up all still index by tier
// (OBJECT_SIZES' own index), not by render position, so this only changes
// visual placement.
//
// Which tiers show up at all now comes from object_scale_exponents(id)
// (packages/pxl/shared's own per-type catalog), not a flat filter applied
// to every swatch alike — OBJECT_SIZES' tier index i is exponent -i (see
// that function's own doc comment for why), so this just flips sign and
// sorts. OBJECT_SIZES itself is untouched (still the single source of
// truth for placement/collision/render) — a type with a narrower active
// set still stays fully functional at its other sizes, it's only hidden
// from this one picker.
function size_tiers_for(id: ObjectColorId): number[] {
  return object_scale_exponents(id).map((exp) => -exp).sort((a, b) => b - a);
}

type SizeFlyoutAnchor = { left: f64; bottom: f64 };

/**
 * Shared press-and-hold size-flyout mechanics — pointer down runs
 * `on_activate` immediately (mouse or touch — plain PointerEvents, not a
 * gesture/animation library's composite drag+layout feature, same
 * reasoning as this app's camera controls being hand-rolled) *and* pops a
 * vertical flyout of the 4 OBJECT_SIZES above whichever button owns this
 * hook instance; dragging up and releasing over one of them also picks
 * that size, while releasing back over the button itself (a plain tap)
 * just runs `on_activate` and leaves whatever size was already active.
 * Hit-testing is done against each item's own measured rect
 * (getBoundingClientRect), not elementFromPoint, so it isn't affected by
 * z-index/overlap from anything else on screen. One instance per button
 * that wants this (see ColorSwatchButton) — each gets its own independent
 * anchor/menu state, tied to its own on-screen position.
 */
function use_size_flyout(on_activate: () => void) {
  const set_object_size = useUiStore((state) => state.set_object_size);

  const [menu_open, set_menu_open] = useState(false);
  const [drag_index, set_drag_index] = useState<number | null>(null);
  // Anchor rect (viewport coords), captured once on pointerdown — the
  // flyout is a portal (see SizeFlyout below), so this is the only link
  // back to the button's on-screen position.
  const [anchor, set_anchor] = useState<SizeFlyoutAnchor | null>(null);
  const item_refs = useRef<(HTMLButtonElement | null)[]>([]);
  const button_ref = useRef<HTMLButtonElement | null>(null);

  function index_at(x: f64, y: f64): number | null {
    for (let i = 0; i < item_refs.current.length; i++) {
      const el = item_refs.current[i];
      if (!el) continue;
      const r = el.getBoundingClientRect();
      if (x >= r.left && x <= r.right && y >= r.top && y <= r.bottom) return i;
    }
    return null;
  }

  function on_pointer_down(e: React.PointerEvent) {
    e.preventDefault();
    on_activate();
    const r = button_ref.current?.getBoundingClientRect();
    if (r) set_anchor({ left: r.left + r.width / 2, bottom: globalThis.innerHeight - r.top + 20 });
    set_menu_open(true);
    set_drag_index(null);

    function on_move(ev: PointerEvent) {
      set_drag_index(index_at(ev.clientX, ev.clientY));
    }
    function on_up(ev: PointerEvent) {
      const idx = index_at(ev.clientX, ev.clientY);
      if (idx !== null) set_object_size(OBJECT_SIZES[idx]);
      set_menu_open(false);
      set_drag_index(null);
      globalThis.removeEventListener("pointermove", on_move);
      globalThis.removeEventListener("pointerup", on_up);
      globalThis.removeEventListener("pointercancel", on_up);
    }
    globalThis.addEventListener("pointermove", on_move);
    globalThis.addEventListener("pointerup", on_up);
    globalThis.addEventListener("pointercancel", on_up);
  }

  return { menu_open, anchor, drag_index, item_refs, button_ref, on_pointer_down };
}

type SizeFlyoutProps = {
  anchor: SizeFlyoutAnchor;
  drag_index: number | null;
  item_refs: React.MutableRefObject<(HTMLButtonElement | null)[]>;
  object_size: f64;
  tiers: readonly number[];
};
/** The flyout itself — one owning button (see use_size_flyout) mounts this
 * while its own menu_open is true; anchor/drag_index/item_refs all come
 * from that same hook instance. `tiers` is that swatch's own active set
 * (size_tiers_for), so each color can show a different range of sizes. */
function SizeFlyout({ anchor, drag_index, item_refs, object_size, tiers }: SizeFlyoutProps) {
  return createPortal(
    // Portaled to document.body instead of an absolutely-positioned
    // sibling of the owning button: that button lives inside the palette
    // pill's overflow-x-auto row, and CSS coerces overflow-y to auto the
    // moment overflow-x isn't visible — so an in-place `absolute
    // bottom-full` flyout got silently clipped by that same scroll box. A
    // fixed-position portal escapes that ancestor entirely.
    <div
      // z-index via inline style, not a Tailwind z-* class — the toast
      // stack (shared/ui/toast/toaster.tsx) portals to document.body too
      // with an inline zIndex: 1000, which sits in the SAME stacking
      // context as this portal (both are direct body children), so a
      // Tailwind z-50 class (z-index: 50) lost to it outright. 2000
      // clears that with margin.
      style={{ position: "fixed", left: anchor.left, bottom: anchor.bottom, transform: "translateX(-50%)", zIndex: 2000 }}
      // Still a dark background (a bare floating column of squares read
      // as adrift when tried without one) but pared down from the full
      // palette-pill treatment (rounded-2xl/p-2.5/border-white/15) that
      // made this read as heavy as the bar it opened from — rounded-full
      // (a vertical capsule, matching the circular buttons inside it),
      // tighter gap/padding, softer border/fill. backdrop-blur-sm +
      // lower opacity (not the solid /90 this started at) matches the
      // other translucent HUD chrome (bottom_overlays.tsx/top_overlays.tsx's
      // own bg-black/40 backdrop-blur-sm) instead of being a one-off
      // solid panel. Same soft shadow as the two main pills (see their own
      // comments) — shadow-lg/black-40 read even heavier on a small
      // translucent popup than it did on the pills themselves.
      className="flex flex-col items-center gap-1.5 rounded-full bg-neutral-900/60 backdrop-blur-sm border border-white/10 shadow-[0_6px_20px_rgba(0,0,0,0.25)] py-2 px-1.5"
    >
      {tiers.map((tier) => {
        const s = OBJECT_SIZES[tier];
        return (
          // Hit target (w-9 h-9) is deliberately much bigger than the
          // visual size square inside it — on mobile the square itself
          // (as small as 8px at the finest tier) is far too small to
          // drag onto reliably. index_at measures THIS button's rect,
          // not the inner square's, so drag/release hit-testing gets
          // the same bigger target the visual gap already implies.
          // rounded-full (not -lg) to match the capsule container.
          <button
            key={s}
            ref={(el) => {
              item_refs.current[tier] = el;
            }}
            type="button"
            className="flex items-center justify-center w-9 h-9 rounded-full shrink-0"
            title={`${s}u`}
          >
            <div
              className={cn(
                "rounded-sm transition-transform",
                size_button_dims(tier),
                drag_index === tier ? "bg-white scale-125 ring-1 ring-white" : object_size === s ? "bg-white/80" : "bg-white/40",
              )}
            />
          </button>
        );
      })}
    </div>,
    document.body,
  );
}

/**
 * One color swatch, doubling as the object tool's own picker the same way
 * the old standalone cube icon used to: a plain tap selects this color (and
 * the object tool) same as before, while press-and-hold pops the size
 * flyout directly above THIS swatch (use_size_flyout) instead of a shared
 * button elsewhere — every swatch is its own independent size-picker
 * anchor, not one button for color and a separate one for size.
 */
function ColorSwatchButton({ id }: { id: ObjectColorId }) {
  const active_tool = useUiStore((state) => state.active_tool);
  const object_color_id = useUiStore((state) => state.object_color_id);
  const object_size = useUiStore((state) => state.object_size);
  const set_active_tool = useUiStore((state) => state.set_active_tool);
  const set_object_color_id = useUiStore((state) => state.set_object_color_id);

  const { menu_open, anchor, drag_index, item_refs, button_ref, on_pointer_down } = use_size_flyout(() => {
    set_object_color_id(id);
    set_active_tool("object");
  });
  const tiers = size_tiers_for(id);

  return (
    // w-4 h-4 flex items-center justify-center: an explicit fixed-size
    // centering box around the button, not just `relative` sized to
    // whatever the button's own content-box happens to be — the selected
    // state's ring-offset/scale-110 button rendered visibly HIGHER than its
    // unselected siblings without this, all still nominally the same w-4
    // h-4 box. Pinning the wrapper to the exact same fixed size and
    // explicitly centering the button inside it removes any room for that
    // per-state box-shadow/transform styling to disagree with the row's own
    // items-center alignment.
    <div className="relative w-4 h-4 flex items-center justify-center">
      {menu_open && anchor && <SizeFlyout anchor={anchor} drag_index={drag_index} item_refs={item_refs} object_size={object_size} tiers={tiers} />}
      <button
        ref={button_ref}
        type="button"
        onPointerDown={on_pointer_down}
        // Flat color swatch face (object_color_hex), not the old
        // wool-texture thumbnail — mode_bar is otherwise entirely
        // grayscale chrome except these swatches, and the texture read as
        // a mismatched level of detail against that flat chrome. The ring
        // is unconditional now (not just on selection) at a faint
        // white/15 — black is nearly invisible against this pill's own
        // bg-neutral-900 without SOME edge to read its boundary against;
        // the selected state's own brighter ring still reads as a clear
        // step up from that.
        // touch-none: without it, a touch-drag here scrolls the page
        // underneath instead of dragging across the flyout above (same
        // reasoning the old cube button's own touch-none had).
        className={cn(
          "w-4 h-4 rounded-full cursor-pointer transition-transform shrink-0 touch-none",
          active_tool === "object" && object_color_id === id ? "ring-1 ring-white ring-offset-1 ring-offset-neutral-900 scale-110" : "ring-1 ring-white/15 scale-100",
        )}
        style={{ backgroundColor: object_color_hex(id) }}
        title={id}
      />
    </div>
  );
}

/**
 * Two separate floating pills (message white, palette dark), not one fused
 * bar. Tapping a collapsed icon morphs that pill wider while
 * the other (if expanded) morphs back to icon-only — tried a single
 * shared-outline bar with a color seam first, read as ambiguous.
 *
 * `rounded-full` on both states (not a fixed radius) is what makes it read
 * as one continuous pill across sizes: collapsed is w-12 h-12 (a true circle,
 * matching this app's other circular controls like compass/GPS), expanded
 * goes much wider — one shape rule for both.
 *
 * `layout` (framer-motion) turns the width/height swap into a smooth spring
 * resize instead of an instant snap. Each pill's own icon (MessageCircle/
 * Palette) is ONE persistent `layout`-tracked element across both states
 * too, not a ternary swapping between a collapsed <button> and a
 * differently-mounted expanded element — same technique toast.tsx's own
 * left icon slot uses (see its doc comment) to stay visually pinned to
 * the pill's left edge through the resize instead of jumping between two
 * unrelated mount points.
 *
 * Nothing here is wired to a real chat system yet — message_text is local,
 * cosmetic-only state; sending only pushes into ui_store.ts's chat_lines
 * log (read by top_overlays.tsx's dummy chat-text preview), it doesn't
 * sync live per keystroke. The palette pill's tool/color selection,
 * however, is real: it drives object placement/erasure (see ui_store.ts's
 * active_tool/object_color_id, read by camera/input.tsx and
 * world/markers/hover_cursor.tsx).
 * Named for what it does (switches between modes — message, palette, and
 * whatever else lands here later) rather than "palette bar", which only ever
 * described one of its two pills.
 */
export function ModeBar() {
  const [mode, set_mode] = useState<ModeId>("message");
  const active_tool = useUiStore((state) => state.active_tool);
  const set_active_tool = useUiStore((state) => state.set_active_tool);
  const append_chat_line = useUiStore((state) => state.append_chat_line);
  const [message_text, set_message_text] = useState("");
  // Sending fades the input out first (opacity transition below) instead
  // of blanking it instantly — is_sending drives that fade, and the actual
  // text clear + chat_lines append happen after SEND_FADE_MS, once the
  // fade has had time to play.
  const [is_sending, set_is_sending] = useState(false);
  const input_ref = useRef<HTMLInputElement | null>(null);

  function send_message() {
    const text = message_text.trim();
    if (text === "" || is_sending) return;
    set_is_sending(true);
    setTimeout(() => {
      append_chat_line(text);
      set_message_text("");
      set_is_sending(false);
      // The send button's own click already moved focus to itself
      // (default button behavior) — pull it back to the input so typing a
      // follow-up message doesn't need a re-tap, same as pressing Enter
      // (which never leaves the input in the first place) already gives.
      input_ref.current?.focus();
    }, SEND_FADE_MS);
  }

  const message_expanded = mode === "message";
  const palette_expanded = mode === "palette";
  // Whenever there's text to send, the palette pill's own (otherwise idle)
  // collapsed-icon slot doubles as the send button instead of a separate
  // one living inside the message pill — see that slot's own comment.
  // palette_expanded is always false here in practice (only one mode is
  // ever active at once), included anyway so this can't show a send
  // button in a slot that's mid-showing real palette content instead.
  const show_send_in_palette_slot = message_expanded && !palette_expanded && message_text.trim() !== "";

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
      {
        /* ONE LayoutGroup around BOTH pills — framer-motion otherwise
        coordinates every `layout`-tracked element on the page through one
        shared projection tree by default (same reasoning toast.tsx's own
        per-instance LayoutGroup gives, see its doc comment: an actively-
        animating layout element could bump/re-project a completely
        unrelated, already-settled element elsewhere on the page right
        along with it). This scopes that isolation to mode_bar as a whole,
        away from unrelated page elements (toasts, etc.) — but deliberately
        does NOT split the two pills into their own separate groups: they
        share this row's justify-center, so one pill resizing genuinely
        does shift where the OTHER pill's center-anchored position lands.
        Splitting them (tried first) isolated their FLIP transforms from
        that shared repositioning too, so each pill's own animation no
        longer accounted for the row-level shift its sibling was causing —
        read as a mismatched, "inertial" drift instead of the two moving
        as one coordinated swap. One shared group is what lets
        framer-motion account for both at once. */
      }
      <LayoutGroup id="mode-bar">
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
            // Soft, low-opacity shadow (toast.tsx's own pill uses the same
            // shape: large blur, low alpha) instead of shadow-lg's harder
            // shadow-black/40 — that read as too heavy/graphic for a chrome
            // element sitting over a busy 3D map. filter: drop-shadow, not a
            // box-shadow class — this element has `layout`, which fakes its
            // width change via a transform: scale() (FLIP) before settling;
            // box-shadow doesn't follow that transform cleanly (it's
            // computed against the box's own geometry, not composited with
            // it), which read as a doubled/ghosted shadow mid-transition.
            // drop-shadow is a filter, so it rides along with the transform
            // like any other pixel this element paints — same reasoning
            // scale_bar.tsx's own outline uses drop-shadow over a real
            // per-element box outline.
            // No overflow-hidden here — deliberately. filter + overflow-
            // hidden on the SAME element is a real cross-browser footgun:
            // the drop-shadow's blur gets clipped hard at the box edge
            // instead of bleeding softly outward, which for any opaque
            // shape inside (like the send button that lives in the palette
            // pill's own version of this wrapper) rendered as a hard dark
            // rim right at its edge instead of a soft ambient shadow — the
            // "black outline" bug. Clipping now lives on the inner wrapper
            // below instead, which carries no filter of its own, so nothing
            // it clips ever fights with a blur trying to bleed past it.
            // No border here either, same reasoning, empirically confirmed
            // live (removing it made the ring vanish instantly): a
            // translucent border rasterized as part of THIS filtered
            // element's own composite, then shadowed, is what actually
            // produced that ring — not just an overflow-clipping issue.
            // The border moved to the inner wrapper below, same as
            // overflow-hidden.
            "flex items-center h-12 bg-white filter-[drop-shadow(0_3px_6px_rgba(0,0,0,0.25))] min-w-0",
          )}
        >
          {
            /* Inner wrapper carries relative + overflow-hidden + the
          pill's own border instead of the outer pill (see its own
          comment for why border/overflow-hidden both moved here).
          mode="popLayout" below makes an exiting child position:
          absolute the instant it starts exiting — without a positioned
          ancestor here, that child anchors to whatever positioned
          ancestor is further up the tree instead (visibly escaping this
          pill entirely), and without overflow-hidden, anything that DOES
          render past this box's own shrinking edge during the transition
          isn't clipped either.
          motion.div with its own `layout` + style-driven borderRadius —
          NOT a plain div with a static `rounded-full` class. The outer
          pill's `layout` fakes its width change via a transform: scale()
          (FLIP) every resize, which visually distorts everything in its
          subtree, including this wrapper's own corners — framer-motion
          only auto-corrects that distortion for elements IT is tracking
          via their own style-prop radius (see the outer pill's matching
          comment on why a Tailwind class is invisible to it); a plain
          child div just inherited the parent's distortion undefended,
          rendering as an oval mid-transition instead of a stable pill.
          Giving this wrapper its own `layout` lets it correct for that
          independently, the same way the outer pill corrects for its own.
          h-full matches whatever height the outer settles on (always h-12
          in practice); pl-2 moved here from the outer pill's own
          className, unrelated to the filter/overflow split above — still
          just the same left inset it always was. */
          }
          <motion.div
            layout
            transition={SPRING}
            style={{ borderRadius: "999px" }}
            className={cn("relative flex items-center h-full overflow-hidden border border-black/10", message_expanded && "pl-2")}
          >
            {
              /* Icon is ONE persistent element (motion.button, `layout` +
            the same SPRING as the pill itself) across both states, not a
            ternary swapping between a collapsed <button> and an expanded
            <div> — that swap used to mean framer-motion had nothing
            continuous to interpolate for the icon itself, so it just
            snapped between two different mount points while the pill's own
            width tweened smoothly around it. This is exactly toast.tsx's
            own technique for its left icon slot (see that file's doc
            comment): same element, `layout`-tracked, same transition as its
            parent — that's what keeps it visually glued to the pill's left
            edge through the whole resize instead of jumping. */
            }
            <motion.button
              type="button"
              layout
              transition={SPRING}
              onClick={() => set_mode("message")}
              className={cn(
                "flex items-center justify-center rounded-full cursor-pointer text-black/50 hover:text-black/80 transition-colors shrink-0",
                // w-12 (not w-14) collapsed: equal to h-12 so rounded-full
                // renders a true circle, matching this app's other circular
                // controls (CircularButton) instead of a stadium/half-pill
                // shape. w-8 expanded: shrinks to a small persistent icon
                // slot beside the input, same size toast.tsx's own icon box
                // uses.
                message_expanded ? "w-8 h-8" : "w-12 h-12",
              )}
              title="message"
            >
              <MessageCircle className="w-4 h-4" strokeWidth={2} />
            </motion.button>

            {
              /* mode="popLayout" — without it, AnimatePresence keeps an
          exiting child occupying its full layout space until ITS OWN exit
          animation finishes, so this pill's own `layout` width couldn't
          actually shrink until that played out — meanwhile the OTHER
          pill's content has no exit to wait through and starts growing
          immediately, reading as "one expands, THEN the other shrinks"
          instead of both moving together. popLayout pulls an exiting
          child out of flow (position: absolute) the instant it starts
          exiting, so the shrink and the sibling's grow both begin on the
          same frame. */
            }
            <AnimatePresence mode="popLayout">
              {message_expanded && (
                <motion.div
                  layout
                  initial={{ opacity: 0, x: -10 }}
                  animate={{ opacity: 1, x: 0 }}
                  exit={{ opacity: 0, x: -10 }}
                  transition={SPRING}
                  className="flex items-center gap-2 min-w-0 pr-2"
                >
                  {
                    /* w-96 (not w-full): 384px is this input's actual
                preferred size, not a lower bound to grow past — max-w-full
                min-w-0 is what lets it shrink under a narrower row, but
                nothing here should ever push it, or the pill around it,
                wider than 384px just because extra space exists. */
                  }
                  <input
                    ref={input_ref}
                    value={message_text}
                    onChange={(e) => set_message_text(e.target.value)}
                    // isComposing guards against a Korean/IME composition's own
                    // Enter (confirming a candidate syllable) also triggering
                    // send — nativeEvent.isComposing is true only mid-composition,
                    // so a real "done typing, send it" Enter still goes through.
                    onKeyDown={(e) => {
                      if (e.key === "Enter" && !e.nativeEvent.isComposing) send_message();
                    }}
                    placeholder="메시지 입력..."
                    autoFocus
                    // opacity fades to 0 over is_sending instead of the text
                    // just vanishing the instant send_message clears it — the
                    // clear itself is delayed by SEND_FADE_MS so it lands right
                    // as this transition finishes, not mid-fade. No send button
                    // in here anymore — see the palette pill's own icon slot,
                    // which doubles as send while this input has text (this
                    // component's own doc comment explains why).
                    className={cn(
                      "bg-transparent text-sm text-black placeholder:text-black/40 outline-none w-96 max-w-full min-w-0 transition-opacity duration-200",
                      is_sending && "opacity-0",
                    )}
                  />
                </motion.div>
              )}
            </AnimatePresence>
          </motion.div>
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
          // max-w caps it at "row width minus margins minus the message
          // pill's own collapsed minimum minus the gap between them," a
          // fixed viewport-relative bound rather than exact flex-remaining-
          // space math (not worth the complexity here) — this pill's content
          // grew a real amount once the size selector added a 4th tier
          // (tools + size squares + 7 color swatches, all in one row), so on
          // a narrow phone it can now genuinely be wider than what's left
          // once the message pill exists too.
          //
          // Only the color-swatch group scrolls (see its own div below) —
          // pointer/eraser + the divider between them stay pinned and always
          // visible, not swept offscreen by a swipe the way they were back
          // when this whole pill shared one overflow-x-auto scroller. This
          // element itself just clips at the pill's own rounded edge
          // (overflow-hidden), it doesn't scroll — and deliberately carries
          // no px-2 of its own: a static padding here would sit OUTSIDE the
          // swatch group's own scroll box, so scrolled-to-the-end swatches
          // would stop short of the pill's true right edge by that fixed
          // gap instead of scrolling flush through it. Left inset (pointer
          // button, never scrolls) comes from a plain pl-2 on the content
          // row below; right inset (swatch group, does scroll) comes from
          // that group's own p-2 instead, so it scrolls through with the
          // content rather than sitting outside it — see their own comments.
          className={cn(
            // Same soft shadow as the message pill above, same reason it's
            // a drop-shadow filter and not a box-shadow class — see that
            // pill's own comment. No overflow-hidden or border here
            // either, same reasoning (confirmed live: removing the border
            // made the ring vanish instantly) — both moved to the inner
            // wrapper below, which carries no filter of its own.
            // No bg-neutral-900 here either, deliberately — this outer
            // pill is now just a shape/shadow carrier, not a persistent
            // backdrop. Every actual visible state (the idle palette
            // circle, the send circle, the expanded tools+swatches row)
            // now paints its own bg-neutral-900 instead of relying on one
            // shared background living on this always-mounted element —
            // a shared background here doesn't fade with whichever
            // foreground state is exiting, so "the icon fades out" left a
            // solid dark circle behind that never went anywhere itself.
            "flex items-center h-12 filter-[drop-shadow(0_3px_6px_rgba(0,0,0,0.25))] shrink-0 max-w-[calc(100vw-7rem)]",
          )}
        >
          {
            /* min-w-0: this new wrapper is itself now a flex item of the
            outer pill — without it, its own implicit min-width:auto would
            block the swatch-scroll chain further inside from ever
            shrinking, the same reason min-w-0 shows up at every other
            level of that chain (see the swatch group's own comment
            below). No border here anymore either, same reasoning as the
            bg-neutral-900 move above — a border on this always-mounted
            wrapper doesn't fade with whichever state is exiting, so
            during mode="wait"'s gap (neither palette nor send mounted)
            this would render as a bare outline ring with nothing inside
            it. Each state below carries its own border instead.
            motion.div with its own `layout` + style-driven borderRadius —
            NOT a plain div with a static `rounded-full` class. Same
            reasoning as the message pill's matching wrapper: the outer
            pill's `layout` fakes its width change via a transform:
            scale() (FLIP), which distorts a plain child's static
            border-radius into an oval mid-transition since nothing
            corrects for it; giving this wrapper its own `layout` lets it
            correct independently instead of inheriting the parent's
            distortion undefended. */
          }
          <motion.div
            layout
            transition={SPRING}
            style={{ borderRadius: "999px" }}
            className="relative flex items-center h-full min-w-0 overflow-hidden"
          >
            {
              /* Unlike the message pill's icon (which persists, just shrunk,
          since nothing else takes its place), this icon has no reason to
          stick around once expanded — pointer/eraser/swatches already
          fill that "which tool" role, so a lingering palette glyph next
          to them was redundant chrome, not a real indicator. Mounted only
          while collapsed; AnimatePresence gives it a real exit (fade +
          shrink) instead of an instant cut when palette_expanded flips.
          mode="popLayout" — same reason as the message pill's own content
          AnimatePresence (see its comment): without it, this exiting icon
          would keep occupying its layout space until its own exit
          animation finished, delaying the pill's shrink relative to
          whatever's growing in its place.
          Doubles as the send button while show_send_in_palette_slot is
          true (message pill expanded + has text) — this slot is already
          just sitting there idle in that state (palette can't be entered
          from a collapsed circle while you're mid-message anyway), so
          repurposing it means one fewer redundant button rather than a
          second send affordance living inside the message pill too.
          Distinct `key`s (not the same element with a conditional icon
          swap) so AnimatePresence gives the palette<->send swap its own
          proper enter/exit, not an instant icon flip mid-box. Clearing
          message_text after a real send (see send_message/SEND_FADE_MS)
          is what flips this back to the palette icon automatically once
          sending finishes. */
            }
            {
              /* Absolutely positioned (not in normal flex flow) so the
          palette<->send swap happens in exactly the same spot instead of
          each one being independently flex-positioned. inset-0 alone (no
          explicit w-12 h-12) stretches each button to fill the wrapper's
          own padding box exactly, whatever it resolves to — a hardcoded
          size here used to independently drift from the wrapper's real
          size (border-box math involving the pill/wrapper's own border)
          and get clipped, showing the pill's dark background through the
          gap.
          `layout` + style-driven borderRadius on BOTH buttons below — not
          just opacity. Confirmed live by sampling the outer pill's actual
          transform mid-resize: it and the wrapper each correctly
          self-compensate their own border-radius against the FLIP
          squish (matching matrix()/border-radius values at every
          sampled frame), but these buttons, with no `layout` of their
          own and a plain static `rounded-full` class, still sit inside
          that squished subtree (a CSS transform on an ancestor visually
          affects every descendant regardless of the descendant's own
          transform) with nothing correcting for it — rendering as an
          oval mid-transition. Same fix as the wrapper's own: give it
          `layout` so framer-motion computes its own compensating radius
          for whatever cumulative squish it inherits.
          mode="wait" (not the default simultaneous/sync mode) — palette
          fully fades out and unmounts BEFORE send starts fading in, one
          after the other, not overlapping. */
            }
            <AnimatePresence mode="wait">
              {!palette_expanded && (
                show_send_in_palette_slot
                  ? (
                    <motion.button
                      key="send"
                      type="button"
                      layout
                      initial={{ opacity: 0 }}
                      animate={{ opacity: 1 }}
                      exit={{ opacity: 0 }}
                      transition={SPRING}
                      style={{ borderRadius: "999px" }}
                      // Stops the button's own default mousedown behavior
                      // (stealing focus to itself) before it ever happens,
                      // instead of only pulling focus back afterward — keeps
                      // a mobile virtual keyboard from flickering closed/open.
                      onMouseDown={(e) => e.preventDefault()}
                      onClick={send_message}
                      disabled={is_sending}
                      className={cn(
                        "absolute inset-0 flex items-center justify-center shrink-0 cursor-pointer border border-white/15",
                        "bg-linear-to-br from-orange-400 to-orange-500 text-white shadow-md shadow-orange-500/40",
                        "transition-all duration-200 hover:from-orange-400 hover:to-orange-600 hover:shadow-lg hover:shadow-orange-500/50",
                        "active:from-orange-500 active:to-orange-700 active:scale-95",
                      )}
                      title="전송"
                    >
                      <Send className="w-4 h-4" strokeWidth={2} />
                    </motion.button>
                  )
                  : (
                    <motion.button
                      key="palette"
                      type="button"
                      layout
                      initial={{ opacity: 0 }}
                      animate={{ opacity: 1 }}
                      exit={{ opacity: 0 }}
                      transition={SPRING}
                      style={{ borderRadius: "999px" }}
                      onClick={() => set_mode("palette")}
                      className="absolute inset-0 flex items-center justify-center cursor-pointer bg-neutral-900 border border-white/15 text-white/60 hover:text-white/90 transition-colors shrink-0"
                      title="palette"
                    >
                      <Palette className="w-4 h-4" strokeWidth={2} />
                    </motion.button>
                  )
              )}
            </AnimatePresence>
            {
              /* Invisible spacer — those no longer participate in this flex
          row's own layout (position: absolute takes them out of flow
          entirely), so without this the pill would have nothing here to
          size itself against while collapsed. w-12 h-full (not h-12) —
          same reasoning as the buttons above: h-full matches the
          wrapper's own actual (border-adjusted) height instead of an
          independent fixed number that could drift from it. */
            }
            {!palette_expanded && <div className="w-12 h-full shrink-0" aria-hidden="true" />}

            {/* mode="popLayout" — see the message pill's own content AnimatePresence for why. */}
            <AnimatePresence mode="popLayout">
              {palette_expanded && (
                // min-w-0 (not shrink-0): it must be ALLOWED to shrink below
                // its content's natural width, or the color-swatch group
                // never gets squeezed enough to trigger its own
                // overflow-x-auto in the first place. No padding lives on
                // this row itself, and none on the vbar either — each of its
                // two real children (tools group, swatch group) carries its
                // own padding independently (px-2 vs the swatch group's
                // wider px-3.5 below — not matched on purpose), so the vbar in
                // between stays a bare 1px line with nothing added around it.
                <motion.div
                  layout
                  initial={{ opacity: 0, x: -10 }}
                  animate={{ opacity: 1, x: 0 }}
                  exit={{ opacity: 0, x: -10 }}
                  transition={SPRING}
                  style={{ borderRadius: "999px" }}
                  className="flex items-center min-w-0 bg-neutral-900 border border-white/15 h-full"
                >
                  {
                    /* Tools group — shrink-0 as a unit so it never gives up
                space to the swatch group. px-2 is this group's own
                inset, independent of the swatch group's own padding below. */
                  }
                  <div className="flex items-center gap-1 px-2 shrink-0">
                    {
                      /* white/20 (not blue) — the very first version of this bar
                  used a neutral white highlight for the selected tool; blue
                  crept in during a later pass and read as an inconsistent
                  one-off accent color for a concept piece that's otherwise
                  entirely grayscale except the color swatches themselves. */
                    }
                    {
                      /* Simple mode toggles (no sub-settings of their own) grouped
                  first; the object tool and its two settings (size flyout,
                  color) grouped second past the divider — previously
                  pointer/object/eraser sat together with color off on its
                  own, which split the object tool from the color swatches
                  that are actually its own setting. */
                    }
                    <button
                      type="button"
                      onClick={() => set_active_tool("pointer")}
                      className={cn(
                        "flex items-center justify-center w-8 h-8 rounded-full cursor-pointer transition-colors shrink-0",
                        active_tool === "pointer" ? "bg-white/20 text-white" : "text-white/60 hover:text-white/90",
                      )}
                      title="포인터"
                    >
                      <MousePointer className="w-4 h-4" strokeWidth={2} />
                    </button>

                    <button
                      type="button"
                      onClick={() => set_active_tool("eraser")}
                      className={cn(
                        "flex items-center justify-center w-8 h-8 rounded-full cursor-pointer transition-colors shrink-0",
                        active_tool === "eraser" ? "bg-white/20 text-white" : "text-white/60 hover:text-white/90",
                      )}
                      title="지우개"
                    >
                      <Eraser className="w-4 h-4" strokeWidth={2} />
                    </button>
                  </div>

                  <div className="w-px h-7 bg-white/15 shrink-0" />

                  {
                    /* No standalone object-tool icon anymore — each color
                swatch below doubles as the object tool's own picker (a
                plain tap selects that color and the tool; press-and-hold
                pops the size flyout right above that swatch) — see
                ColorSwatchButton's own doc comment. py-2 keeps the same
                8px vertical inset the hand button's own 32px height needs
                (dots are w-4 h-4 (16px), 16 + 8 + 8 = 32); px-3.5 is wider
                on the horizontal axis on purpose, not required to match
                py-2 anymore. Padding here (not a margin/gap upstream of this
                div) is what puts space between the divider and the first
                swatch, and between the last swatch and the pill's own
                edge — since it's on the scrolling element itself, it
                scrolls through cleanly instead of sitting outside the
                scroll box as a fixed gap (same reasoning on both sides).
                min-w-0 lets this group shrink below its content's natural
                width (the tools group above is shrink-0 and never gives up
                space, so this is the only part of the row that can) —
                overflow-x-auto then turns that shrunk space into a
                scroller, this group's own edge (not the outer pill's) is
                now the scroll boundary. Scrollbar hidden via the two rules
                below; overscroll-x-contain stops a swipe here from also
                scrolling the page behind it. */
                  }
                  <div className="flex items-center gap-3.5 px-3.5 py-2 min-w-0 overflow-x-auto overscroll-x-contain [scrollbar-width:none] [&::-webkit-scrollbar]:hidden">
                    {object_color_ids.map((id) => <ColorSwatchButton key={id} id={id} />)}
                  </div>
                </motion.div>
              )}
            </AnimatePresence>
          </motion.div>
        </motion.div>
      </LayoutGroup>
    </div>
  );
}
