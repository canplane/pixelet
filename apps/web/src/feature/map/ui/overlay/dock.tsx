// @pxl/web/feature/map/ui/overlay/dock.tsx

import { useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Keyboard, PanResponder, PixelRatio, Pressable, ScrollView, StyleSheet, type TextInput, View } from "react-native";
import { PAN_X } from "@platform/gesture.ts";

import type { f64, u32, u8 } from "@xpute/core/abi/word.ts";
import { round } from "@xpute/core/math/scalar.ts";
import { Backing } from "@platform/backing.tsx";
import type { GlyphName } from "@platform/icon.tsx";
import { BackingPlace } from "@/shared/ui/backing.ts";
import { Glyph } from "@/shared/ui/glyph.tsx";
import { Motion } from "@platform/motion.tsx";
import { LABEL } from "@platform/palette.ts";
import { Pill } from "@platform/pill.tsx";
import { Presence, Swap } from "@/shared/ui/motion/presence.tsx";
import { FADE, LEAVE, SNAP } from "@/shared/ui/motion/transition.ts";
import { keepKeyboard } from "@/shared/ui/keyboard.tsx";
import { TextField } from "@/shared/ui/text-field.tsx";
import { alpha, chrome, color, material, shadow } from "@/shared/ui/theme.ts";
import { openModal } from "@/app/modal.tsx";
import { holdTier, holdTool, useInHand, useUiStore } from "@/app/store/ui.ts";
import { useKernel } from "@/kernel/shell.ts";
import { ActiveTool } from "@/kernel/abi/cmd.spec.ts";
import { type PlaceableCube, placeableCubeId } from "@/catalog/cell/mod.ts";
import { OBJECT_TIERS } from "@/config/object.spec.ts";
import { cubeLabel, CubeSwatch, decorGlyph, decorLabel, heldRing } from "@/catalog/cell/swatch.tsx";

const SEND_FROM_SCALE = 0.5;

const ITEM = 24;
const ITEM_GLYPH = 16;

const FLYOUT_LIFT_PX = 20;
const FLYOUT_GAP_PX = 8;
const FLYOUT_PAD_Y = 8;
const FLYOUT_WIDTH = chrome.target + 12;
const FLYOUT_FOOT = (chrome.control + ITEM) / 2 + FLYOUT_LIFT_PX;
const FLYOUT_HOLD_MS = 150;
const FLYOUT_PITCH = chrome.target + FLYOUT_GAP_PX;
/** The first move this far decides: up opens the sizes, sideways scrolls the strip. */
const FLYOUT_AXIS_PX = 12;

const enum Gesture {
  Pending = 0,
  Flyout = 1,
  Scroll = 2,
}

/** The dock's pills, in bar order; one is open and the rest fold to a button. */
const enum Pane {
  Message = 0,
  Palette = 1,
}
const PANES: readonly Pane[] = [Pane.Message, Pane.Palette];

const enum Slot {
  Empty = 0,
  Palette = 1,
  Send = 2,
}

/** A dot whose area halves per tier, so the finest stays visible: 28, 20, 14, 10. */
const MARK_COARSEST = 28;
function sizeMark(tier: u32): f64 {
  return round(MARK_COARSEST / Math.SQRT2 ** tier);
}

/** Finest first, so the coarsest stands nearest the swatch. */
const TIERS: readonly u32[] = Array.from({ length: OBJECT_TIERS }, (_, i) => OBJECT_TIERS - 1 - i);

interface Flyout {
  left: f64;
  centerX: f64;
  foot: f64;
  hover: u32 | null;
}

function tierAt(at: Flyout, x: f64, y: f64): u32 | null {
  if (Math.abs(x - at.centerX) > FLYOUT_WIDTH / 2) return null;
  const top = at.foot - (2 * FLYOUT_PAD_Y + OBJECT_TIERS * FLYOUT_PITCH - FLYOUT_GAP_PX);
  const along = y - top - FLYOUT_PAD_Y;
  if (along < 0) return null;
  const row = Math.floor(along / FLYOUT_PITCH);
  if (row >= OBJECT_TIERS || along - row * FLYOUT_PITCH > chrome.target) return null;
  return TIERS[row];
}

/** Drawn from the bar, not the swatch, whose scrolling strip would clip it. */
function SizeFlyout({ hover }: { hover: u32 | null }) {
  const { t } = useTranslation();
  const objectTier = useKernel((s) => s.holding.tier);

  return (
    <Backing fill={STRIP.ground} material={material.flyout} place={BackingPlace.OverMap} style={styles.flyout}>
      {TIERS.map((tier) => {
        const mark = sizeMark(tier);
        return (
          <View key={tier} accessibilityLabel={t("map.dock.object_size", { n: OBJECT_TIERS - tier })} style={styles.flyoutItem}>
            <Motion to={{ scale: hover === tier ? 1.25 : 1 }} transition={SNAP}>
              <View
                style={[
                  { width: mark, height: mark, borderRadius: mark / 2 },
                  hover === tier ? styles.markHover : objectTier === tier ? styles.markHeld : styles.markIdle,
                ]}
              />
            </Motion>
          </View>
        );
      })}
    </Backing>
  );
}

interface SizeGesture {
  tap: () => void;
  open: (swatch: View) => void;
  move: (x: f64, y: f64) => void;
  lift: (x: f64, y: f64) => void;
  close: () => void;
}

/** A tap takes the cube; a hold or a drag up opens the sizes; a drag sideways is the strip's. */
function useSizeGesture(swatch: React.RefObject<View | null>, g: SizeGesture) {
  const handlers = useRef(g);
  handlers.current = g;
  const responder = useRef<ReturnType<typeof PanResponder.create> | null>(null);
  if (!responder.current) {
    let state = Gesture.Pending;
    let hold: ReturnType<typeof setTimeout> | null = null;
    const stopHold = () => {
      if (hold !== null) clearTimeout(hold);
      hold = null;
    };
    const open = () => {
      stopHold();
      if (state !== Gesture.Pending || !swatch.current) return;
      state = Gesture.Flyout;
      handlers.current.open(swatch.current);
    };
    responder.current = PanResponder.create({
      onStartShouldSetPanResponder: () => true,
      onPanResponderGrant: () => {
        state = Gesture.Pending;
        hold = setTimeout(open, FLYOUT_HOLD_MS);
      },
      onPanResponderMove: (_e, s) => {
        if (state === Gesture.Flyout) return handlers.current.move(s.moveX, s.moveY);
        if (state === Gesture.Scroll) return;
        const dx = Math.abs(s.dx);
        const dy = Math.abs(s.dy);
        if (dx < FLYOUT_AXIS_PX && dy < FLYOUT_AXIS_PX) return;
        if (dy >= dx) {
          open();
          handlers.current.move(s.moveX, s.moveY);
        } else {
          state = Gesture.Scroll;
          stopHold();
        }
      },
      onPanResponderRelease: (_e, s) => {
        stopHold();
        if (state === Gesture.Flyout) handlers.current.lift(s.moveX, s.moveY);
        else if (state === Gesture.Pending) handlers.current.tap();
        state = Gesture.Pending;
      },
      onPanResponderTerminate: () => {
        stopHold();
        if (state === Gesture.Flyout) handlers.current.close();
        state = Gesture.Pending;
      },
      onPanResponderTerminationRequest: () => state !== Gesture.Flyout,
    });
  }
  return responder.current.panHandlers;
}

function SwatchButton({ cube, sizes }: { cube: PlaceableCube; sizes: Omit<SizeGesture, "tap"> }) {
  const { t } = useTranslation();
  const selectCube = useUiStore.getState().selectCube;
  const selected = useInHand({ tool: ActiveTool.Object, cube });
  const ref = useRef<View | null>(null);

  const handlers = useSizeGesture(ref, {
    ...sizes,
    tap: () => selectCube(cube),
    open: (swatch) => {
      selectCube(cube);
      sizes.open(swatch);
    },
  });

  return (
    <View ref={ref} {...handlers} accessibilityRole="button" accessibilityLabel={cubeLabel(t, cube)} style={[styles.slot, styles.swatchSlot]}>
      <Motion to={{ scale: selected ? 1.1 : 1 }} transition={SNAP}>
        <CubeSwatch cube={cube} size={ITEM} radius={ITEM / 2} style={selected ? styles.swatchSelected : styles.swatchIdle} />
      </Motion>
    </View>
  );
}

function DecorIconButton({ kind }: { kind: u8 }) {
  const { t } = useTranslation();
  const selectDecor = useUiStore.getState().selectDecor;
  const active = useInHand({ tool: ActiveTool.Decor, kind });

  return (
    <Pressable onPress={() => selectDecor(kind)} accessibilityRole="button" accessibilityLabel={decorLabel(t, kind)} style={styles.slot}>
      <Motion to={{ scale: active ? 1.1 : 1 }} transition={SNAP}>
        <View style={[styles.decorDisc, active ? styles.swatchSelected : styles.swatchIdle]}>
          <Glyph glyph={decorGlyph(kind)} size={ITEM_GLYPH} color={active ? STRIP.glyphOn : STRIP.glyph} />
        </View>
      </Motion>
    </Pressable>
  );
}

function ToolButton({ label, glyph, active, onPress }: { label: string; glyph: GlyphName; active?: boolean; onPress: () => void }) {
  return (
    <Pressable onPress={onPress} accessibilityRole="button" accessibilityLabel={label} style={styles.slot}>
      {
        /* Always mounted: Android, creating it once lit, drew its ground without
                its corners. */
      }
      <View collapsable={false} style={[styles.held, active && styles.toolActive]}>
        <Glyph glyph={glyph} size={chrome.glyph} color={active ? STRIP.glyphOn : STRIP.glyph} />
      </View>
    </Pressable>
  );
}

export function Dock() {
  const { t } = useTranslation();
  const [open, setOpen] = useState<Pane>(Pane.Palette);
  // A downward drag on the bar puts the keyboard away; swatches only open
  // upward, so the two gestures never compete.
  const putAway = useRef(
    PanResponder.create({
      onStartShouldSetPanResponderCapture: keepKeyboard,
      onMoveShouldSetPanResponderCapture: (_e, s) => s.dy > FLYOUT_AXIS_PX && s.dy > Math.abs(s.dx),
      onPanResponderGrant: () => Keyboard.dismiss(),
    }),
  ).current;
  const activeTool = useKernel((s) => s.holding.tool);
  const recentPicks = useUiStore((s) => s.recentPicks);
  const appendChatLine = useUiStore.getState().appendChatLine;
  const [messageText, setMessageText] = useState("");
  const [isSending, setIsSending] = useState(false);
  const [barWidth, setBarWidth] = useState<f64>(0);
  const [stripWidth, setStripWidth] = useState<f64>(chrome.control);
  const [flyout, setFlyout] = useState<Flyout | null>(null);
  const inputRef = useRef<TextInput | null>(null);
  const bar = useRef<View | null>(null);

  function sendMessage() {
    const text = messageText.trim();
    if (text === "" || isSending) return;
    setIsSending(true);
    setTimeout(() => {
      appendChatLine(text);
      setMessageText("");
      setIsSending(false);
      inputRef.current?.focus();
    }, FADE.durationMs);
  }

  const sizes: Omit<SizeGesture, "tap"> = {
    open: (swatch) => {
      swatch.measureInWindow((sx, _sy, sw) => {
        bar.current?.measureInWindow((bx, by) => {
          setFlyout({ left: sx + sw / 2 - bx, centerX: sx + sw / 2, foot: by + chrome.control - FLYOUT_FOOT, hover: null });
        });
      });
    },
    move: (x, y) => {
      if (flyout === null) return;
      const hover = tierAt(flyout, x, y);
      if (hover !== flyout.hover) setFlyout({ ...flyout, hover });
    },
    lift: (x, y) => {
      const tier = flyout === null ? null : tierAt(flyout, x, y);
      if (tier !== null) holdTier(tier);
      setFlyout(null);
    },
    close: () => setFlyout(null),
  };

  const openFlyout = open === Pane.Palette ? flyout : null;
  // The send button takes the folded palette's place.
  const slot = open === Pane.Palette ? Slot.Empty : messageText.trim() !== "" ? Slot.Send : Slot.Palette;
  const pillColor = slot === Slot.Send ? color.send : STRIP.ground;

  const room = Math.max(chrome.control, barWidth - (PANES.length - 1) * (chrome.control + chrome.controlGap));
  // Every pane opens as wide as the palette's tools.
  const opened = Math.min(stripWidth, room);
  const widths = PANES.map((pane) => (pane === open ? opened : chrome.control));
  const xs: f64[] = [];
  // Whole pixels, where a page's pill cuts its shadow cleanly.
  let x = (barWidth - widths.reduce((sum, w) => sum + w, 0) - (PANES.length - 1) * chrome.controlGap) / 2;
  for (const w of widths) {
    xs.push(PixelRatio.roundToNearestPixel(x));
    x += w + chrome.controlGap;
  }

  return (
    <View ref={bar} {...putAway.panHandlers} onLayout={(e) => setBarWidth(e.nativeEvent.layout.width)} style={styles.bar}>
      {barWidth > 0 && (
        <>
          <Pill
            x={xs[Pane.Message]}
            width={widths[Pane.Message]}
            height={chrome.control}
            span={barWidth}
            fill={color.ground}
            place={BackingPlace.OverMap}
            interactive={open !== Pane.Message}
            shadow={shadow.lift}
          >
            <Pressable
              onPress={() => setOpen(Pane.Message)}
              accessibilityRole="button"
              accessibilityLabel={t("map.dock.message_tooltip")}
              style={styles.messageIcon}
            >
              <Glyph glyph="bubble.left" size={chrome.glyph} color={LABEL.primary} />
            </Pressable>
            <Presence
              show={open === Pane.Message}
              from={{ opacity: 0, translateX: -10 }}
              exit={{ opacity: 0, translateX: -10 }}
              transition={SNAP}
              style={[styles.messageField, { width: opened - chrome.control - 8 }]}
            >
              <Motion to={{ opacity: isSending ? 0 : 1 }} transition={FADE} style={styles.fieldFill}>
                <TextField
                  ref={inputRef}
                  value={messageText}
                  onChangeText={setMessageText}
                  onSubmit={sendMessage}
                  placeholder={t("map.dock.message_placeholder")}
                  placeholderTextColor={LABEL.placeholder}
                  autoFocus
                  style={styles.messageInput}
                />
              </Motion>
            </Presence>
          </Pill>

          <Pill
            x={xs[Pane.Palette]}
            width={widths[Pane.Palette]}
            height={chrome.control}
            span={barWidth}
            fill={pillColor}
            tint={pillColor}
            place={BackingPlace.OverMap}
            interactive={open !== Pane.Palette}
            shadow={shadow.lift}
            onContentWidth={setStripWidth}
          >
            <Motion
              to={open === Pane.Palette ? {} : { opacity: 0, translateX: -10 }}
              transition={SNAP}
              style={[styles.strip, { maxWidth: room }, open === Pane.Palette ? styles.pass : styles.block]}
            >
              <View style={styles.group}>
                <ToolButton label={t("map.dock.pointer")} glyph="hand.point.up.left" active={activeTool === ActiveTool.Pointer} onPress={() => holdTool(ActiveTool.Pointer)} />
                <ToolButton label={t("map.dock.eraser")} glyph="eraser" active={activeTool === ActiveTool.Eraser} onPress={() => holdTool(ActiveTool.Eraser)} />
              </View>

              <View style={styles.divider} />

              <ScrollView
                horizontal
                scrollEnabled={flyout === null}
                showsHorizontalScrollIndicator={false}
                style={styles.picks}
                contentContainerStyle={styles.picksContent}
              >
                {recentPicks.map((pick) =>
                  pick.tool === ActiveTool.Object ? <SwatchButton key={placeableCubeId(pick.cube)} cube={pick.cube} sizes={sizes} /> : <DecorIconButton key={`decor:${pick.kind}`} kind={pick.kind} />
                )}
              </ScrollView>

              <View style={styles.divider} />

              <View style={styles.group}>
                <ToolButton label={t("map.dock.drawer_tooltip")} glyph="square.grid.2x2" onPress={() => openModal("drawer")} />
              </View>
            </Motion>

            <Swap
              id={slot}
              from={slot === Slot.Send ? { opacity: 0, scale: SEND_FROM_SCALE } : { opacity: 0 }}
              exit={{ opacity: 0 }}
              transition={slot === Slot.Send ? SNAP : LEAVE}
              exitTransition={LEAVE}
              style={[styles.slotButton, { pointerEvents: open === Pane.Palette ? "none" : "auto" }]}
            >
              {slot === Slot.Empty ? null : slot === Slot.Send
                ? (
                  <Pressable
                    onPress={sendMessage}
                    disabled={isSending}
                    accessibilityRole="button"
                    accessibilityLabel={t("map.dock.send")}
                    style={({ pressed }) => [styles.slotButton, pressed && styles.sendPressed]}
                  >
                    <Glyph glyph="arrow.up" size={chrome.glyph} weight="bold" color={color.white} />
                  </Pressable>
                )
                : (
                  <Pressable
                    // Not blurred here: blurring on press snapped the pills to the palette on a
                    // device instead of animating.
                    onPress={() => setOpen(Pane.Palette)}
                    accessibilityRole="button"
                    accessibilityLabel={t("map.dock.palette_tooltip")}
                    style={styles.slotButton}
                  >
                    <Glyph glyph="paintpalette" size={chrome.glyph} color={STRIP.glyph} />
                  </Pressable>
                )}
            </Swap>
          </Pill>

          {openFlyout && (
            <View style={[styles.flyoutAt, { left: openFlyout.left - FLYOUT_WIDTH / 2 }]}>
              <SizeFlyout hover={openFlyout.hover} />
            </View>
          )}
        </>
      )}
    </View>
  );
}

const PILL = { height: chrome.control, borderRadius: chrome.control / 2 };

const STRIP = {
  glyph: alpha(color.white, 0.6),
  glyphOn: color.white,
  mark: color.white,
  held: alpha(color.white, 0.2),
  line: alpha(color.white, 0.15),
  swatchLine: alpha(color.white, 0.3),
  ground: color.band,
};

const styles = StyleSheet.create({
  bar: { height: chrome.control, width: "100%", minWidth: 0, pointerEvents: "box-none" },

  messageIcon: { width: chrome.control, height: chrome.control, alignItems: "center", justifyContent: "center" },
  messageField: { flexDirection: "row", alignItems: "center", minWidth: 0 },
  fieldFill: { flex: 1 },
  messageInput: { width: "100%", color: LABEL.primary },

  slotButton: { ...PILL, position: "absolute", left: 0, top: 0, width: chrome.control, alignItems: "center", justifyContent: "center" },
  sendPressed: { opacity: 0.7 },

  strip: { ...PILL, flexDirection: "row", alignItems: "center" },
  // An inline `box-none` is drawn as a `none` the strip's tools inherit.
  pass: { pointerEvents: "box-none" },
  block: { pointerEvents: "none" },
  group: { flexDirection: "row", alignItems: "center", padding: (chrome.control - chrome.target) / 2, flexShrink: 0 },
  divider: { width: 1, height: 28, backgroundColor: STRIP.line, flexShrink: 0 },
  picks: { flexShrink: 1, minWidth: 0 },
  picksContent: { alignItems: "center", paddingHorizontal: (chrome.control - chrome.target) / 2 },

  // No wider than a target: past it, the later neighbor in the tree takes the tap.
  slot: { width: chrome.target, height: chrome.target, alignItems: "center", justifyContent: "center", flexShrink: 0 },
  swatchSlot: PAN_X,
  held: { width: chrome.selection, height: chrome.selection, borderRadius: chrome.selection / 2, alignItems: "center", justifyContent: "center" },
  toolActive: { backgroundColor: STRIP.held },
  decorDisc: { width: ITEM, height: ITEM, borderRadius: ITEM / 2, alignItems: "center", justifyContent: "center", backgroundColor: STRIP.held },
  swatchIdle: { boxShadow: `0 0 0 1px ${STRIP.swatchLine}` },
  swatchSelected: { boxShadow: heldRing(STRIP.ground, STRIP.mark, 1) },

  // Takes the pointer whole, so the map under it draws no hover.
  flyoutAt: {
    position: "absolute",
    bottom: FLYOUT_FOOT,
    width: FLYOUT_WIDTH,
    zIndex: 10,
    pointerEvents: "box-only",
  },
  flyout: {
    alignItems: "center",
    gap: FLYOUT_GAP_PX,
    paddingVertical: FLYOUT_PAD_Y,
    borderRadius: FLYOUT_WIDTH / 2,
    boxShadow: shadow.lift,
  },
  flyoutItem: { width: chrome.target, height: chrome.target, alignItems: "center", justifyContent: "center" },
  markIdle: { backgroundColor: alpha(STRIP.mark, 0.4) },
  markHeld: { backgroundColor: alpha(STRIP.mark, 0.8) },
  markHover: { backgroundColor: STRIP.mark, boxShadow: `0 0 0 1px ${STRIP.mark}` },
});
