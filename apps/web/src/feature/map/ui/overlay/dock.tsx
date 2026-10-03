// @pxl/web/feature/map/ui/overlay/dock.tsx

import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Keyboard, PanResponder, PixelRatio, Pressable, ScrollView, StyleSheet, type TextInput, View } from "react-native";
import { PAN_X } from "@platform/gesture.ts";

import type { f64, i32, u32, u8 } from "@xpute/core/abi/word.ts";
import { round } from "@xpute/core/math/scalar.ts";
import { Backing } from "@platform/backing.tsx";
import type { GlyphName } from "@platform/icon.tsx";
import { BackingPlace } from "@/shared/ui/backing.ts";
import { Glyph } from "@/shared/ui/glyph.tsx";
import { Motion } from "@platform/motion.tsx";
import { LABEL } from "@platform/palette.ts";
import { Pill } from "@platform/pill.tsx";
import { cardPose, deckDepth, deckSwipe, peekBelow, useTurnedBack } from "@/shared/ui/deck.ts";
import { Presence } from "@/shared/ui/motion/presence.tsx";
import { FADE, SETTLE, SNAP } from "@/shared/ui/motion/transition.ts";
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
const PEEK_PX = 10;

/** The dock stands tall enough for the card behind to peek out under the front one. */
export const DOCK_HEIGHT = chrome.control + PEEK_PX;

/** The bar's swipe takes the card's drags; this only keeps a touch on it off the map. */
const BAR_TOUCH = {};

const ITEM = 24;
const ITEM_GLYPH = 16;

/** Above the card, where the sizes and their seed stand. */
const FLYOUT_LIFT_PX = 6;
const FLYOUT_GAP_PX = 8;
const FLYOUT_PAD_Y = 8;
const FLYOUT_WIDTH = chrome.target + 12;
const FLYOUT_FOOT = chrome.control + FLYOUT_LIFT_PX;
const FLYOUT_HOLD_MS = 300;
const FLYOUT_PITCH = chrome.target + FLYOUT_GAP_PX;
/** A move this far before the hold is a drag: the strip's sideways, the deck's up and down. */
const DRAG_PX = 12;

const enum Gesture {
  Pending = 0,
  Flyout = 1,
  Drag = 2,
}

/** The dock's cards, in deck order. */
const enum Pane {
  Message = 0,
  Palette = 1,
}
const PANES: readonly Pane[] = [Pane.Message, Pane.Palette];

/** A dot whose area halves per tier, so the finest stays visible: 28, 20, 14, 10. */
const MARK_COARSEST = 28;
function sizeMark(tier: u32): f64 {
  return round(MARK_COARSEST / Math.SQRT2 ** tier);
}

/** Finest first, so the coarsest stands nearest the swatch. */
const TIERS: readonly u32[] = Array.from({ length: OBJECT_TIERS }, (_, i) => OBJECT_TIERS - 1 - i);

/** A press on a swatch shows the sizes' seed; held, it grows into them. */
const enum Sizes {
  Seed = 0,
  Open = 1,
}

/** The seed of the sizes, at their foot. */
const SEED = 10;

interface Flyout {
  left: f64;
  centerX: f64;
  foot: f64;
  hover: u32 | null;
  sizes: Sizes;
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
  press: (swatch: View) => void;
  open: () => void;
  move: (x: f64, y: f64) => void;
  lift: (x: f64, y: f64) => void;
  close: () => void;
}

/** A tap takes the cube; a hold opens the sizes. */
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
      if (state !== Gesture.Pending) return;
      state = Gesture.Flyout;
      handlers.current.open();
    };
    responder.current = PanResponder.create({
      onStartShouldSetPanResponder: () => true,
      onPanResponderGrant: () => {
        state = Gesture.Pending;
        if (swatch.current) handlers.current.press(swatch.current);
        hold = setTimeout(open, FLYOUT_HOLD_MS);
      },
      onPanResponderMove: (_e, s) => {
        if (state === Gesture.Flyout) return handlers.current.move(s.moveX, s.moveY);
        if (state === Gesture.Drag || (Math.abs(s.dx) < DRAG_PX && Math.abs(s.dy) < DRAG_PX)) return;
        state = Gesture.Drag;
        stopHold();
        handlers.current.close();
      },
      onPanResponderRelease: (_e, s) => {
        stopHold();
        if (state === Gesture.Flyout) handlers.current.lift(s.moveX, s.moveY);
        else if (state === Gesture.Pending) handlers.current.tap();
        handlers.current.close();
        state = Gesture.Pending;
      },
      onPanResponderTerminate: () => {
        stopHold();
        handlers.current.close();
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
    open: () => {
      selectCube(cube);
      sizes.open();
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
  const [rotation, setRotation] = useState<i32>(Pane.Palette);
  const turnedBack = useTurnedBack(rotation);
  // A vertical drag on the bar turns the deck; swatches open their sizes on a
  // hold, so the two never compete.
  const swipe = useRef(
    PanResponder.create({
      onStartShouldSetPanResponderCapture: keepKeyboard,
      onMoveShouldSetPanResponderCapture: (_e, s) => Math.abs(s.dy) > DRAG_PX && Math.abs(s.dy) > Math.abs(s.dx),
      ...deckSwipe((delta) => setRotation((r) => r + delta)),
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
  // A release can come before the swatch's place is measured.
  const pressing = useRef(false);
  const inputRef = useRef<TextInput | null>(null);
  const bar = useRef<View | null>(null);

  const front = PANES.find((pane) => deckDepth(pane, rotation, PANES.length) === 0);
  // The message card in front is for typing; turned away, the keyboard goes with it.
  useEffect(() => {
    if (front === Pane.Message) inputRef.current?.focus();
    else Keyboard.dismiss();
  }, [front]);

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
    press: (swatch) => {
      pressing.current = true;
      swatch.measureInWindow((sx, _sy, sw) => {
        bar.current?.measureInWindow((bx, by) => {
          if (!pressing.current) return;
          setFlyout({ left: sx + sw / 2 - bx, centerX: sx + sw / 2, foot: by + chrome.control - FLYOUT_FOOT, hover: null, sizes: Sizes.Seed });
        });
      });
    },
    open: () => setFlyout((f) => f && { ...f, sizes: Sizes.Open }),
    move: (x, y) => {
      if (flyout?.sizes !== Sizes.Open) return;
      const hover = tierAt(flyout, x, y);
      if (hover !== flyout.hover) setFlyout({ ...flyout, hover });
    },
    lift: (x, y) => {
      const tier = flyout?.sizes === Sizes.Open ? tierAt(flyout, x, y) : null;
      if (tier !== null) holdTier(tier);
    },
    close: () => {
      pressing.current = false;
      setFlyout(null);
    },
  };

  const shownFlyout = front === Pane.Palette ? flyout : null;
  // Every card is as wide as the palette's tools; whole pixels, where a page's pill cuts its shadow cleanly.
  const width = Math.min(stripWidth, barWidth);
  const x = PixelRatio.roundToNearestPixel((barWidth - width) / 2);
  const canSend = messageText.trim() !== "";

  const card = (pane: Pane, fill: string, tint: string | undefined, content: React.ReactNode, onContentWidth?: (w: f64) => void) => {
    const depth = deckDepth(pane, rotation, PANES.length);
    const pose = cardPose(depth, PEEK_PX);
    const fromAbove = depth === 0 && turnedBack;
    return (
      <Motion
        key={pane}
        from={fromAbove ? pose.above : pose.rest}
        replay={fromAbove ? rotation : null}
        to={pose.rest}
        transition={SETTLE}
        style={[styles.card, depth === 0 ? styles.pass : styles.block, { zIndex: pose.zIndex }]}
      >
        <Pill
          x={x}
          width={width}
          height={chrome.control}
          span={barWidth}
          fill={fill}
          tint={tint}
          place={BackingPlace.OverMap}
          shadow={shadow.lift}
          fade={pose.fade}
          fadeTransition={SETTLE}
          touch={depth === 0 ? BAR_TOUCH : undefined}
          touchBelow={depth === 0 ? peekBelow(PANES.length, PEEK_PX) : 0}
          onContentWidth={onContentWidth}
        >
          {content}
        </Pill>
      </Motion>
    );
  };

  return (
    <View ref={bar} {...swipe.panHandlers} onLayout={(e) => setBarWidth(e.nativeEvent.layout.width)} style={styles.bar}>
      {barWidth > 0 && (
        <>
          {card(
            Pane.Message,
            color.ground,
            undefined,
            <View style={[styles.message, { width }]}>
              <View style={styles.messageIcon}>
                <Glyph glyph="bubble.left" size={chrome.glyph} color={LABEL.primary} />
              </View>
              <Motion to={{ opacity: isSending ? 0 : 1 }} transition={FADE} style={styles.fieldFill}>
                <TextField
                  ref={inputRef}
                  value={messageText}
                  onChangeText={setMessageText}
                  onSubmit={sendMessage}
                  placeholder={t("map.dock.message_placeholder")}
                  placeholderTextColor={LABEL.placeholder}
                  style={styles.messageInput}
                />
              </Motion>
              <Presence show={canSend} from={{ opacity: 0, scale: SEND_FROM_SCALE }} exit={{ opacity: 0 }} transition={SNAP} style={styles.slot}>
                <Pressable
                  onPress={sendMessage}
                  disabled={isSending}
                  accessibilityRole="button"
                  accessibilityLabel={t("map.dock.send")}
                  style={({ pressed }) => [styles.send, pressed && styles.sendPressed]}
                >
                  <Glyph glyph="arrow.up" size={chrome.glyph} weight="bold" color={color.white} />
                </Pressable>
              </Presence>
            </View>,
          )}

          {card(
            Pane.Palette,
            STRIP.ground,
            STRIP.ground,
            <View style={[styles.strip, { maxWidth: barWidth }]}>
              <View style={styles.group}>
                <ToolButton label={t("map.dock.pointer")} glyph="hand.point.up.left" active={activeTool === ActiveTool.Pointer} onPress={() => holdTool(ActiveTool.Pointer)} />
                <ToolButton label={t("map.dock.eraser")} glyph="eraser" active={activeTool === ActiveTool.Eraser} onPress={() => holdTool(ActiveTool.Eraser)} />
              </View>

              <View style={styles.divider} />

              <ScrollView
                horizontal
                scrollEnabled={flyout?.sizes !== Sizes.Open}
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
            </View>,
            setStripWidth,
          )}

          {shownFlyout && (
            <View style={[styles.flyoutAt, shownFlyout.sizes === Sizes.Open ? styles.flyoutTakes : styles.block, { left: shownFlyout.left - FLYOUT_WIDTH / 2 }]}>
              {shownFlyout.sizes === Sizes.Open
                ? <SizeFlyout hover={shownFlyout.hover} />
                : <Backing fill={STRIP.ground} material={material.flyout} place={BackingPlace.OverMap} style={styles.seed} />}
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
  bar: { height: DOCK_HEIGHT, width: "100%", minWidth: 0, pointerEvents: "box-none" },

  card: { position: "absolute", left: 0, top: 0, right: 0, height: chrome.control, transformOrigin: "center bottom" },
  // An inline `box-none` is drawn as a `none` the card's controls inherit.
  pass: { pointerEvents: "box-none" },
  block: { pointerEvents: "none" },

  message: { flexDirection: "row", alignItems: "center", height: chrome.control, paddingRight: (chrome.control - chrome.target) / 2 },
  messageIcon: { width: chrome.control, height: chrome.control, alignItems: "center", justifyContent: "center" },
  fieldFill: { flex: 1, minWidth: 0 },
  messageInput: { width: "100%", color: LABEL.primary },
  send: { width: chrome.selection, height: chrome.selection, borderRadius: chrome.selection / 2, alignItems: "center", justifyContent: "center", backgroundColor: color.send },
  sendPressed: { opacity: 0.7 },

  strip: { ...PILL, flexDirection: "row", alignItems: "center" },
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

  flyoutAt: {
    position: "absolute",
    bottom: PEEK_PX + FLYOUT_FOOT,
    width: FLYOUT_WIDTH,
    zIndex: 10,
  },
  // Takes the pointer whole, so the map under it draws no hover.
  flyoutTakes: { pointerEvents: "box-only" },
  seed: { alignSelf: "center", width: SEED, height: SEED, borderRadius: SEED / 2, boxShadow: shadow.lift },
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
