// @pxl/web/feature/map/ui/overlay/mode-bar.tsx

import { useRef, useState } from "react";
import { createPortal } from "react-dom";
import { AnimatePresence, LayoutGroup, motion } from "framer-motion";
import { Eraser, LayoutGrid, MessageCircle, MousePointer, Palette, Send, TreePine } from "lucide-react";
import { useTranslation } from "react-i18next";

import type { f64, u32 } from "@xpute/core/abi/word.ts";
import { cn } from "@/shared/lib/util/cn.ts";
import { TextField } from "@/shared/ui/text-field.tsx";
import { useAppContext } from "@/app/provider/context.ts";
import { useUiStore } from "@/app/store/ui-store.ts";
import { type DecorId, placeable_cubes, type PlaceableCube, placeableCubeId } from "@/catalog/cell/mod.ts";
import { OBJECT_SIZES, objectScaleExponents } from "@/feature/map/object/size.ts";
import { cubeSwatchStyle } from "./cube-swatch.ts";

type ModeId = "message" | "palette";

const SPRING = { type: "spring" as const, stiffness: 500, damping: 35, mass: 0.6 };

const SWAP_FADE = { ...SPRING, opacity: { duration: 0.12, ease: "easeOut" as const } };

const SEND_FADE_MS = 200;

// Mirrors the h-12 pills below (Tailwind's JIT scanner can't take this as a
// variable, so it stays a literal class there — see this file's own note a
// few lines down) — app/layout.tsx reads this to keep Toaster clear of
// wherever BottomOverlays ends up placing this bar.
export const MODE_BAR_HEIGHT_PX = 48;

// Static strings, not a computed w-[Npx] — Tailwind's JIT scanner needs
// literal classes to generate them.
function sizeButtonDims(i: u32): string {
  return i === 0 ? "w-6 h-6" : i === 1 ? "w-4 h-4" : i === 2 ? "w-2 h-2" : "w-1 h-1";
}

// Tier index i is exponent -i (objectScaleExponents), sign-flipped and
// sorted here for display only — itemRefs/dragIndex/on_up still index by
// tier (OBJECT_SIZES' own index), not by render position.
function sizeTiersFor(cube: PlaceableCube): f64[] {
  return objectScaleExponents(cube).map((exp) => -exp).sort((a, b) => b - a);
}

interface SizeFlyoutAnchor {
  left: f64;
  bottom: f64;
}

const FLYOUT_HOLD_MS = 150;
const FLYOUT_AXIS_PX = 12;

function useSizeFlyout(onActivate: () => void) {
  const setObjectSize = useUiStore.getState().setObjectSize;

  const [menuOpen, setMenuOpen] = useState(false);
  const [dragIndex, setDragIndex] = useState<number | null>(null);
  const [anchor, setAnchor] = useState<SizeFlyoutAnchor | null>(null);
  const itemRefs = useRef<(HTMLButtonElement | null)[]>([]);
  const buttonRef = useRef<HTMLElement | null>(null);

  function indexAt(x: f64, y: f64): u32 | null {
    for (let i = 0; i < itemRefs.current.length; i++) {
      const el = itemRefs.current[i];
      if (!el) continue;
      const r = el.getBoundingClientRect();
      if (x >= r.left && x <= r.right && y >= r.top && y <= r.bottom) return i;
    }
    return null;
  }

  function onPointerDown(e: React.PointerEvent) {
    const startX = e.clientX;
    const startY = e.clientY;

    let gesture: "flyout" | "scroll" | null = null;

    function openFlyout() {
      if (gesture !== null) return;
      gesture = "flyout";
      onActivate();
      const r = buttonRef.current?.getBoundingClientRect();
      if (r) setAnchor({ left: r.left + r.width / 2, bottom: globalThis.innerHeight - r.top + 20 });
      setMenuOpen(true);
      setDragIndex(null);
    }

    const holdTimer = globalThis.setTimeout(openFlyout, FLYOUT_HOLD_MS);

    function cleanup() {
      globalThis.clearTimeout(holdTimer);
      globalThis.removeEventListener("pointermove", on_move);
      globalThis.removeEventListener("pointerup", on_up);
      globalThis.removeEventListener("pointercancel", onCancel);
    }

    function on_move(ev: PointerEvent) {
      if (gesture === "flyout") {
        setDragIndex(indexAt(ev.clientX, ev.clientY));
        return;
      }
      if (gesture === "scroll") return;

      const dx = Math.abs(ev.clientX - startX);
      const dy = Math.abs(ev.clientY - startY);
      if (dx < FLYOUT_AXIS_PX && dy < FLYOUT_AXIS_PX) return;

      if (dy >= dx) {
        globalThis.clearTimeout(holdTimer);
        openFlyout();
        setDragIndex(indexAt(ev.clientX, ev.clientY));
      } else {
        gesture = "scroll";
        globalThis.clearTimeout(holdTimer);
      }
    }

    function on_up(ev: PointerEvent) {
      if (gesture === "flyout") {
        const idx = indexAt(ev.clientX, ev.clientY);
        if (idx !== null) setObjectSize(OBJECT_SIZES[idx]);
      } else if (gesture === null) {
        onActivate();
      }
      setMenuOpen(false);
      setDragIndex(null);
      cleanup();
    }

    function onCancel() {
      setMenuOpen(false);
      setDragIndex(null);
      cleanup();
    }

    globalThis.addEventListener("pointermove", on_move);
    globalThis.addEventListener("pointerup", on_up);
    globalThis.addEventListener("pointercancel", onCancel);
  }

  return { menuOpen, anchor, dragIndex, itemRefs, buttonRef, onPointerDown };
}

interface SizeFlyoutProps {
  anchor: SizeFlyoutAnchor;
  dragIndex: u32 | null;
  itemRefs: React.MutableRefObject<(HTMLButtonElement | null)[]>;
  object_size: f64;
  tiers: readonly f64[];
}

function SizeFlyout({ anchor, dragIndex, itemRefs, object_size, tiers }: SizeFlyoutProps) {
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
              itemRefs.current[tier] = el;
            }}
            type="button"
            className="flex items-center justify-center w-10 h-10 rounded-full shrink-0"
            title={`${s}u`}
          >
            <div
              className={cn(
                "rounded-sm transition-transform",
                sizeButtonDims(tier),
                dragIndex === tier ? "bg-white scale-125 ring-1 ring-white" : object_size === s ? "bg-white/80" : "bg-white/40",
              )}
            />
          </button>
        );
      })}
    </div>,
    document.body,
  );
}

function ColorSwatchButton({ cube }: { cube: PlaceableCube }) {
  const { t } = useTranslation();
  const activeTool = useUiStore((s) => s.activeTool);
  const objectColorId = useUiStore((s) => s.objectColorId);
  const object_size = useUiStore((s) => s.object_size);
  const selectCube = (pick: PlaceableCube) => useUiStore.getState().selectCube(placeableCubeId(pick));

  const { menuOpen, anchor, dragIndex, itemRefs, buttonRef, onPointerDown } = useSizeFlyout(() => selectCube(cube));
  const tiers = sizeTiersFor(cube);

  return (
    <div className="relative w-[38px] h-[38px] flex items-center justify-center">
      {menuOpen && anchor && <SizeFlyout anchor={anchor} dragIndex={dragIndex} itemRefs={itemRefs} object_size={object_size} tiers={tiers} />}
      <button
        type="button"
        onPointerDown={onPointerDown}
        className="w-[38px] h-[38px] flex items-center justify-center cursor-pointer shrink-0 touch-pan-x"
        title={t(`cubes.${placeableCubeId(cube)}`)}
      >
        <span
          ref={buttonRef}
          style={cubeSwatchStyle(cube)}
          className={cn(
            "w-5 h-5 rounded-full transition-transform",
            activeTool === "object" && objectColorId === placeableCubeId(cube) ? "ring-1 ring-white ring-offset-1 ring-offset-neutral-900 scale-110" : "ring-1 ring-white/15 scale-100",
          )}
        />
      </button>
    </div>
  );
}

// The hotbar's decor slot(s) — reinstated (2026-09-07) now that recentPicks
// (ui-store.ts) is one shared MRU list across cube and decor: unlike a
// ColorSwatchButton, a decor pick has no size flyout of its own (DECOR_SIZE
// is fixed, object/size.ts), so this stays a plain tap button.
function DecorIconButton({ id }: { id: DecorId }) {
  const { t } = useTranslation();
  const activeTool = useUiStore((s) => s.activeTool);
  const decor_id = useUiStore((s) => s.decor_id);
  const selectDecor = useUiStore.getState().selectDecor;
  const Icon = DECOR_ICONS[id];
  const active = activeTool === "decor" && decor_id === id;

  return (
    <button
      type="button"
      onClick={() => selectDecor(id)}
      className={cn(
        "flex items-center justify-center w-[38px] h-[38px] rounded-full cursor-pointer transition-colors shrink-0",
        active ? "bg-white/20 text-white" : "text-white/60 hover:text-white/90",
      )}
      title={t(`decors.${id}`)}
    >
      <Icon className="w-5 h-5" strokeWidth={2} />
    </button>
  );
}

// Icon per decor id — TypeScript enforces one entry per DecorId, so
// adding a new decor kind (decor.ts's own decor_types) can't silently ship
// without a button icon for it. Lives here (this file's own DecorIconButton
// is one consumer) and is also imported by drawer-modal.tsx's own
// DecorGridButton, so a new decor kind's icon shows up in both the hotbar
// and the drawer for free.
export const DECOR_ICONS: Record<DecorId, typeof TreePine> = {
  tree: TreePine,
};

export function ModeBar() {
  const { t } = useTranslation();
  const { ui } = useAppContext();
  const [mode, setMode] = useState<ModeId>("palette");
  const activeTool = useUiStore((s) => s.activeTool);
  const set_activeTool = useUiStore.getState().set_activeTool;
  const recentPicks = useUiStore((s) => s.recentPicks);
  const append_chat_line = useUiStore.getState().append_chat_line;
  const [messageText, setMessageText] = useState("");
  const [isSending, setIsSending] = useState(false);
  const inputRef = useRef<HTMLInputElement | null>(null);

  function sendMessage() {
    const text = messageText.trim();
    if (text === "" || isSending) return;
    setIsSending(true);
    setTimeout(() => {
      append_chat_line(text);
      setMessageText("");
      setIsSending(false);
      inputRef.current?.focus();
    }, SEND_FADE_MS);
  }

  const messageExpanded = mode === "message";
  const paletteExpanded = mode === "palette";
  // Reuses the palette pill's own idle collapsed-icon slot as the send
  // button rather than adding a separate one inside the message pill — the
  // same idle-slot-reuse pattern as the size flyout anchoring per-swatch
  // instead of to one shared control (see AGENTS.md).
  const showSendInPaletteSlot = messageExpanded && !paletteExpanded && messageText.trim() !== "";

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
            className={cn("relative flex items-center h-full overflow-hidden", messageExpanded && "pl-2")}
          >
            <motion.button
              type="button"
              layout
              transition={SPRING}
              onClick={() => setMode("message")}
              className={cn(
                "flex items-center justify-center rounded-full cursor-pointer text-black/50 hover:text-black/80 transition-colors shrink-0",
                messageExpanded ? "w-9 h-9" : "h-full aspect-square",
              )}
              title={t("map.mode_bar.message_tooltip")}
            >
              <MessageCircle className="w-5 h-5" strokeWidth={2} />
            </motion.button>

            {
              /*
              mode="popLayout" takes the exiting child out of flow so the
              pill can shrink around it — without it the pill never shrinks
              on collapse at all (measured).

              The entering child must NOT also carry `layout`. It has no
              previous box on the frame it mounts, so it joins the projection
              tree as a fresh lead and suppresses the pill's own layout
              animation. That reads as: the pill springs open the first time
              and snaps open every time after, because the state only
              persists once a popLayout exit has happened. Collapse keeps
              working throughout, which is what makes it look intermittent
              rather than broken. Measured: 48 -> 436 in one frame on every
              expand after the first.

              The child is the thing whose mount *causes* the size change.
              Its own transition is the opacity/x fade; the box belongs to
              the pill.
            */
            }
            <AnimatePresence mode="popLayout">
              {messageExpanded && (
                <motion.div
                  initial={{ opacity: 0, x: -10 }}
                  animate={{ opacity: 1, x: 0 }}
                  exit={{ opacity: 0, x: -10 }}
                  transition={SPRING}
                  className="flex items-center gap-2 min-w-0 pr-2"
                >
                  <TextField
                    ref={inputRef}
                    value={messageText}
                    onChange={(e) => setMessageText(e.target.value)}
                    onSubmit={sendMessage}
                    placeholder={t("map.mode_bar.message_placeholder")}
                    autoFocus
                    className={cn(
                      "bg-transparent text-black placeholder:text-black/40 outline-none w-96 max-w-full min-w-0 transition-opacity duration-200",
                      isSending && "opacity-0",
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
              {!paletteExpanded && (
                showSendInPaletteSlot
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
                      onClick={sendMessage}
                      disabled={isSending}
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
                      onClick={() => setMode("palette")}
                      className="absolute inset-0 flex items-center justify-center cursor-pointer bg-neutral-900 text-white/60 hover:text-white/90 transition-colors shrink-0"
                      title={t("map.mode_bar.palette_tooltip")}
                    >
                      <Palette className="w-5 h-5" strokeWidth={2} />
                    </motion.button>
                  )
              )}
            </AnimatePresence>
            {!paletteExpanded && <div className="w-12 h-full shrink-0" aria-hidden="true" />}

            {
              /*
              mode="popLayout" takes the exiting child out of flow so the
              pill can shrink around it — without it the pill never shrinks
              on collapse at all (measured).

              The entering child must NOT also carry `layout`. It has no
              previous box on the frame it mounts, so it joins the projection
              tree as a fresh lead and suppresses the pill's own layout
              animation. That reads as: the pill springs open the first time
              and snaps open every time after, because the state only
              persists once a popLayout exit has happened. Collapse keeps
              working throughout, which is what makes it look intermittent
              rather than broken. Measured: 48 -> 436 in one frame on every
              expand after the first.

              The child is the thing whose mount *causes* the size change.
              Its own transition is the opacity/x fade; the box belongs to
              the pill.
            */
            }
            <AnimatePresence mode="popLayout">
              {paletteExpanded && (
                <motion.div
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
                    {recentPicks.map((pick) =>
                      pick.kind === "object"
                        ? <ColorSwatchButton key={`object:${pick.id}`} cube={placeable_cubes.find((c) => placeableCubeId(c) === pick.id)!} />
                        : <DecorIconButton key={`decor:${pick.id}`} id={pick.id} />
                    )}
                  </div>

                  <div className="w-px h-7 bg-white/15 shrink-0" />

                  {
                    // Opens the full catalog: every terrain-styled block
                    // type and decor kind that hasn't (yet) earned a hotbar
                    // slot, plus Character — Minecraft's own hotbar/
                    // inventory relationship. Outside the scrolling strip
                    // above so it's always reachable regardless of scroll
                    // position.
                  }
                  <div className="flex items-center p-[4px] shrink-0">
                    <button
                      type="button"
                      onClick={ui.openDrawerModal}
                      className="flex items-center justify-center w-[38px] h-[38px] rounded-full cursor-pointer text-white/60 hover:text-white/90 transition-colors shrink-0"
                      title={t("map.mode_bar.catalog_tooltip")}
                    >
                      <LayoutGrid className="w-5 h-5" strokeWidth={2} />
                    </button>
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
