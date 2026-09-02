// @pxl/web/modules/map/ui/overlay/mode_bar.tsx

import { useRef, useState } from "react";
import { createPortal } from "react-dom";
import { AnimatePresence, LayoutGroup, motion } from "framer-motion";
import { Eraser, MessageCircle, MousePointer, Palette, Send } from "lucide-react";
import { useTranslation } from "react-i18next";

import type { f64 } from "@xpute/core/kit/word.ts";
import { cn } from "@/shared/lib/utils/cn.ts";
import { useUiStore } from "@/app/store/ui_store.ts";
import { object_color_hex, object_color_ids, object_scale_exponents, type ObjectColorId } from "@pxl/shared/spec/cell/objects.ts";
import { OBJECT_SIZES } from "@/modules/world/objects/size.ts";

type ModeId = "message" | "palette";

const SPRING = { type: "spring" as const, stiffness: 500, damping: 35, mass: 0.6 };

const SWAP_FADE = { ...SPRING, opacity: { duration: 0.12, ease: "easeOut" as const } };

const SEND_FADE_MS = 200;

// Mirrors the h-12 pills below (Tailwind's JIT scanner can't take this as a
// variable, so it stays a literal class there — see this file's own note a
// few lines down) — app_layout.tsx reads this to keep Toaster clear of
// wherever BottomOverlays ends up placing this bar.
export const MODE_BAR_HEIGHT_PX = 48;

// Static strings, not a computed w-[Npx] — Tailwind's JIT scanner needs
// literal classes to generate them.
function size_button_dims(i: number): string {
  return i === 0 ? "w-6 h-6" : i === 1 ? "w-4 h-4" : i === 2 ? "w-2 h-2" : "w-1 h-1";
}

// Tier index i is exponent -i (object_scale_exponents), sign-flipped and
// sorted here for display only — item_refs/drag_index/on_up still index by
// tier (OBJECT_SIZES' own index), not by render position.
function size_tiers_for(id: ObjectColorId): number[] {
  return object_scale_exponents(id).map((exp) => -exp).sort((a, b) => b - a);
}

type SizeFlyoutAnchor = { left: f64; bottom: f64 };

const FLYOUT_HOLD_MS = 150;
const FLYOUT_AXIS_PX = 12;

function use_size_flyout(on_activate: () => void) {
  const set_object_size = useUiStore((state) => state.set_object_size);

  const [menu_open, set_menu_open] = useState(false);
  const [drag_index, set_drag_index] = useState<number | null>(null);
  const [anchor, set_anchor] = useState<SizeFlyoutAnchor | null>(null);
  const item_refs = useRef<(HTMLButtonElement | null)[]>([]);
  const button_ref = useRef<HTMLElement | null>(null);

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
    const start_x = e.clientX;
    const start_y = e.clientY;

    let gesture: "flyout" | "scroll" | null = null;

    function open_flyout() {
      if (gesture !== null) return;
      gesture = "flyout";
      on_activate();
      const r = button_ref.current?.getBoundingClientRect();
      if (r) set_anchor({ left: r.left + r.width / 2, bottom: globalThis.innerHeight - r.top + 20 });
      set_menu_open(true);
      set_drag_index(null);
    }

    const hold_timer = globalThis.setTimeout(open_flyout, FLYOUT_HOLD_MS);

    function cleanup() {
      globalThis.clearTimeout(hold_timer);
      globalThis.removeEventListener("pointermove", on_move);
      globalThis.removeEventListener("pointerup", on_up);
      globalThis.removeEventListener("pointercancel", on_cancel);
    }

    function on_move(ev: PointerEvent) {
      if (gesture === "flyout") {
        set_drag_index(index_at(ev.clientX, ev.clientY));
        return;
      }
      if (gesture === "scroll") return;

      const dx = Math.abs(ev.clientX - start_x);
      const dy = Math.abs(ev.clientY - start_y);
      if (dx < FLYOUT_AXIS_PX && dy < FLYOUT_AXIS_PX) return;

      if (dy >= dx) {
        globalThis.clearTimeout(hold_timer);
        open_flyout();
        set_drag_index(index_at(ev.clientX, ev.clientY));
      } else {
        gesture = "scroll";
        globalThis.clearTimeout(hold_timer);
      }
    }

    function on_up(ev: PointerEvent) {
      if (gesture === "flyout") {
        const idx = index_at(ev.clientX, ev.clientY);
        if (idx !== null) set_object_size(OBJECT_SIZES[idx]);
      } else if (gesture === null) {
        on_activate();
      }
      set_menu_open(false);
      set_drag_index(null);
      cleanup();
    }

    function on_cancel() {
      set_menu_open(false);
      set_drag_index(null);
      cleanup();
    }

    globalThis.addEventListener("pointermove", on_move);
    globalThis.addEventListener("pointerup", on_up);
    globalThis.addEventListener("pointercancel", on_cancel);
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

function SizeFlyout({ anchor, drag_index, item_refs, object_size, tiers }: SizeFlyoutProps) {
  return createPortal(
    // Portaled to document.body, not positioned relative to the owning
    // button: that button sits in a horizontally scrolling row, and CSS
    // coerces overflow-y to auto the moment overflow-x isn't visible, so an
    // in-place flyout would get silently clipped by that same scroll box.
    <div
      // z-index 2000 via inline style — the toast stack also portals
      // straight to document.body with an inline zIndex: 1000, the same
      // stacking context as this portal, so a Tailwind z-50 class would
      // lose to it outright.
      style={{ position: "fixed", left: anchor.left, bottom: anchor.bottom, transform: "translateX(-50%)", zIndex: 2000 }}
      className="flex flex-col items-center gap-1.5 rounded-full bg-neutral-900/60 backdrop-blur-sm shadow-[0_6px_20px_rgba(0,0,0,0.25)] py-2 px-1.5"
    >
      {tiers.map((tier) => {
        const s = OBJECT_SIZES[tier];
        return (
          <button
            key={s}
            ref={(el) => {
              item_refs.current[tier] = el;
            }}
            type="button"
            className="flex items-center justify-center w-10 h-10 rounded-full shrink-0"
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

function ColorSwatchButton({ id }: { id: ObjectColorId }) {
  const { t } = useTranslation();
  const activeTool = useUiStore((state) => state.activeTool);
  const object_color_id = useUiStore((state) => state.object_color_id);
  const object_size = useUiStore((state) => state.object_size);
  const set_activeTool = useUiStore((state) => state.set_activeTool);
  const set_object_color_id = useUiStore((state) => state.set_object_color_id);

  const { menu_open, anchor, drag_index, item_refs, button_ref, on_pointer_down } = use_size_flyout(() => {
    set_object_color_id(id);
    set_activeTool("object");
  });
  const tiers = size_tiers_for(id);

  return (
    <div className="relative w-[38px] h-[38px] flex items-center justify-center">
      {menu_open && anchor && <SizeFlyout anchor={anchor} drag_index={drag_index} item_refs={item_refs} object_size={object_size} tiers={tiers} />}
      <button
        type="button"
        onPointerDown={on_pointer_down}
        className="w-[38px] h-[38px] flex items-center justify-center cursor-pointer shrink-0 touch-pan-x"
        title={t(`object_colors.${id}`)}
      >
        <span
          ref={button_ref}
          style={{ backgroundColor: object_color_hex(id) }}
          className={cn(
            "w-5 h-5 rounded-full transition-transform",
            activeTool === "object" && object_color_id === id ? "ring-1 ring-white ring-offset-1 ring-offset-neutral-900 scale-110" : "ring-1 ring-white/15 scale-100",
          )}
        />
      </button>
    </div>
  );
}

export function ModeBar() {
  const { t } = useTranslation();
  const [mode, set_mode] = useState<ModeId>("palette");
  const activeTool = useUiStore((state) => state.activeTool);
  const set_activeTool = useUiStore((state) => state.set_activeTool);
  const append_chat_line = useUiStore((state) => state.append_chat_line);
  const [message_text, set_message_text] = useState("");
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
      input_ref.current?.focus();
    }, SEND_FADE_MS);
  }

  const message_expanded = mode === "message";
  const palette_expanded = mode === "palette";
  // Reuses the palette pill's own idle collapsed-icon slot as the send
  // button rather than adding a separate one inside the message pill — the
  // same idle-slot-reuse pattern as the size flyout anchoring per-swatch
  // instead of to one shared control (see AGENTS.md).
  const show_send_in_palette_slot = message_expanded && !palette_expanded && message_text.trim() !== "";

  return (
    <div className="flex items-center justify-center gap-2.5 min-w-0 w-full">
      <LayoutGroup id="mode-bar">
        <motion.div
          layout
          transition={SPRING}
          // borderRadius via style, not a rounded-full class: an element
          // with `layout` fakes width changes via transform: scale()
          // (FLIP), and framer-motion only auto-corrects a shape's radius
          // against that squish when the radius comes through its own
          // tracked `style` prop — a Tailwind class is invisible to it and
          // renders as an oval mid-transition. Applies to every
          // `layout`-tracked motion.div below that sets this same style.
          style={{ borderRadius: "999px" }}
          className={cn(
            // box-shadow, not filter: drop-shadow — this element is an
            // opaque rectangle, and filters render badly over the WebGL
            // canvas in WebKit (see AGENTS.md's Overlay CSS section).
            "flex items-center h-12 bg-white shadow-[0_6px_20px_rgba(0,0,0,0.25)] min-w-0",
          )}
        >
          <motion.div
            layout
            transition={SPRING}
            style={{ borderRadius: "999px" }}
            className={cn("relative flex items-center h-full overflow-hidden", message_expanded && "pl-2")}
          >
            <motion.button
              type="button"
              layout
              transition={SPRING}
              onClick={() => set_mode("message")}
              className={cn(
                "flex items-center justify-center rounded-full cursor-pointer text-black/50 hover:text-black/80 transition-colors shrink-0",
                message_expanded ? "w-9 h-9" : "h-full aspect-square",
              )}
              title={t("map.mode_bar.message_tooltip")}
            >
              <MessageCircle className="w-5 h-5" strokeWidth={2} />
            </motion.button>

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
                  <input
                    ref={input_ref}
                    value={message_text}
                    onChange={(e) => set_message_text(e.target.value)}
                    // isComposing guards against a Korean/IME composition's
                    // own Enter (confirming a candidate syllable) also
                    // triggering send.
                    onKeyDown={(e) => {
                      if (e.key === "Enter" && !e.nativeEvent.isComposing) send_message();
                    }}
                    placeholder={t("map.mode_bar.message_placeholder")}
                    autoFocus
                    className={cn(
                      // text-base on mobile, not text-sm: iOS auto-zooms
                      // the page when a focused input's font-size is under
                      // 16px, with no way to opt out that doesn't also
                      // disable pinch-zoom entirely.
                      "bg-transparent text-base md:text-sm text-black placeholder:text-black/40 outline-none w-96 max-w-full min-w-0 transition-opacity duration-200",
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
          style={{ borderRadius: "999px" }}
          className={cn(
            // filter: drop-shadow stays here, unlike its twin above — this
            // element has no background of its own, so the shape being
            // shadowed is whatever its children currently render (each
            // painting its own rounded background). box-shadow would draw
            // one rectangle around the lot. This is the exception AGENTS.md's
            // rule carves out: filter for a genuinely irregular shape.
            "flex items-center h-12 filter-[drop-shadow(0_3px_6px_rgba(0,0,0,0.25))] shrink-0 max-w-[calc(100vw-5.75rem)]",
          )}
        >
          <motion.div
            layout
            transition={SPRING}
            style={{ borderRadius: "999px" }}
            className="relative flex items-center h-full min-w-0 overflow-hidden"
          >
            <AnimatePresence>
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
                      transition={SWAP_FADE}
                      style={{ borderRadius: "999px" }}
                      // Stops the button's default mousedown focus-steal
                      // before it happens, so it doesn't flicker a mobile
                      // virtual keyboard closed/open.
                      onMouseDown={(e) => e.preventDefault()}
                      onClick={send_message}
                      disabled={is_sending}
                      className={cn(
                        "absolute inset-0 flex items-center justify-center shrink-0 cursor-pointer",
                        "bg-linear-to-br from-orange-400 to-orange-500 text-white shadow-md shadow-orange-500/40",
                        "transition-all duration-200 hover:from-orange-400 hover:to-orange-600 hover:shadow-lg hover:shadow-orange-500/50",
                        "active:from-orange-500 active:to-orange-700 active:scale-95",
                      )}
                      title={t("map.mode_bar.send")}
                    >
                      <Send className="w-5 h-5" strokeWidth={2} />
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
                      transition={SWAP_FADE}
                      style={{ borderRadius: "999px" }}
                      onClick={() => set_mode("palette")}
                      className="absolute inset-0 flex items-center justify-center cursor-pointer bg-neutral-900 text-white/60 hover:text-white/90 transition-colors shrink-0"
                      title={t("map.mode_bar.palette_tooltip")}
                    >
                      <Palette className="w-5 h-5" strokeWidth={2} />
                    </motion.button>
                  )
              )}
            </AnimatePresence>
            {!palette_expanded && <div className="w-12 h-full shrink-0" aria-hidden="true" />}

            <AnimatePresence mode="popLayout">
              {palette_expanded && (
                <motion.div
                  layout
                  initial={{ opacity: 0, x: -10 }}
                  animate={{ opacity: 1, x: 0 }}
                  exit={{ opacity: 0, x: -10 }}
                  transition={SPRING}
                  style={{ borderRadius: "999px" }}
                  className="flex items-center min-w-0 bg-neutral-900 h-full"
                >
                  <div className="flex items-center p-[4px] shrink-0">
                    <button
                      type="button"
                      onClick={() => set_activeTool("pointer")}
                      className={cn(
                        "flex items-center justify-center w-[38px] h-[38px] rounded-full cursor-pointer transition-colors shrink-0",
                        activeTool === "pointer" ? "bg-white/20 text-white" : "text-white/60 hover:text-white/90",
                      )}
                      title={t("map.mode_bar.pointer")}
                    >
                      <MousePointer className="w-5 h-5" strokeWidth={2} />
                    </button>

                    <button
                      type="button"
                      onClick={() => set_activeTool("eraser")}
                      className={cn(
                        "flex items-center justify-center w-[38px] h-[38px] rounded-full cursor-pointer transition-colors shrink-0",
                        activeTool === "eraser" ? "bg-white/20 text-white" : "text-white/60 hover:text-white/90",
                      )}
                      title={t("map.mode_bar.eraser")}
                    >
                      <Eraser className="w-5 h-5" strokeWidth={2} />
                    </button>
                  </div>

                  <div className="w-px h-7 bg-white/15 shrink-0" />

                  <div className="flex items-center px-[4px] min-w-0 overflow-x-auto overscroll-x-contain [scrollbar-width:none] [&::-webkit-scrollbar]:hidden">
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
