// @pxl/web/feature/map/ui/overlay/mode-bar.tsx

import { useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Keyboard, PanResponder, PixelRatio, Pressable, ScrollView, StyleSheet, type TextInput, View } from "react-native";
import { PAN_X } from "@platform/gesture.ts";

import type { f64, u32, u8 } from "@xpute/core/abi/word.ts";
import { round } from "@xpute/core/math/scalar.ts";
import { Backing } from "@platform/backing.tsx";
import { BackingPlace } from "@/shared/ui/backing-place.ts";
import { Glyph } from "@/shared/ui/glyph.tsx";
import { Motion } from "@platform/motion.tsx";
import { LABEL } from "@platform/palette.ts";
import { Pill } from "@platform/pill.tsx";
import { Presence, Swap } from "@/shared/ui/motion/presence.tsx";
import { FADE, LEAVE, SNAP } from "@/shared/ui/motion/transition.ts";
import { keepKeyboard } from "@/shared/ui/keyboard.tsx";
import { TextField } from "@/shared/ui/text-field.tsx";
import { alpha, chrome, color, material, shadow } from "@/shared/ui/theme.ts";
import { useAppContext } from "@/app/provider/context.ts";
import { useInHand, useUiStore } from "@/app/store/ui.ts";
import { ActiveTool } from "@/kernel/abi/cmd.spec.ts";
import { decorName, type PlaceableCube, placeableCubeId } from "@/catalog/cell/mod.ts";
import { useKernel } from "@/kernel/shell.ts";
import { cubeLabel, CubeSwatch, DECOR_GLYPH, heldRing } from "./cube-swatch.tsx";

const SEND_FROM_SCALE = 0.5;

export const MODE_BAR_HEIGHT_PX = chrome.control;
/** A tap target is as wide as the pitch and no wider: past it, the later
 * neighbor in the tree takes the tap. */
const PITCH = chrome.target;
const ITEM = 24;
const ITEM_GLYPH = 16;
const HELD_PX = chrome.selection;
const PILL_GAP = chrome.controlGap;
const FIELD_PX = 384;
const MESSAGE_OPEN_PX = MODE_BAR_HEIGHT_PX + FIELD_PX + 16;

const FLYOUT_LIFT_PX = 20;
const FLYOUT_ITEM_PX = chrome.target;
const FLYOUT_GAP_PX = 8;
const FLYOUT_PAD_Y = 8;
const FLYOUT_WIDTH = FLYOUT_ITEM_PX + 12;
const FLYOUT_FOOT = (MODE_BAR_HEIGHT_PX + ITEM) / 2 + FLYOUT_LIFT_PX;
const FLYOUT_HOLD_MS = 150;
/** The first move this far decides: up opens the sizes, sideways scrolls the strip. */
const FLYOUT_AXIS_PX = 12;

const enum Gesture {
  Pending = 0,
  Flyout = 1,
  Scroll = 2,
}

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
function sizeTiers(count: u32): u32[] {
  return Array.from({ length: count }, (_, i) => count - 1 - i);
}

interface Flyout {
  left: f64;
  centerX: f64;
  foot: f64;
  hover: u32 | null;
}

function tierAt(at: Flyout, count: u32, x: f64, y: f64): u32 | null {
  if (Math.abs(x - at.centerX) > FLYOUT_WIDTH / 2) return null;
  const top = at.foot - (2 * FLYOUT_PAD_Y + count * FLYOUT_ITEM_PX + (count - 1) * FLYOUT_GAP_PX);
  const along = y - top - FLYOUT_PAD_Y;
  if (along < 0) return null;
  const row = Math.floor(along / (FLYOUT_ITEM_PX + FLYOUT_GAP_PX));
  if (row >= count || along - row * (FLYOUT_ITEM_PX + FLYOUT_GAP_PX) > FLYOUT_ITEM_PX) return null;
  return sizeTiers(count)[row];
}

/** Drawn from the bar, not the swatch, whose scrolling strip would clip it. */
function SizeFlyout({ hover }: { hover: u32 | null }) {
  const objectTier = useUiStore((s) => s.objectTier);
  const sizes = useKernel((s) => s.object_sizes);

  return (
    <Backing fill={STRIP.ground} material={material.flyout} place={BackingPlace.OverMap} style={styles.flyout}>
      {sizeTiers(sizes.length).map((tier) => {
        const mark = sizeMark(tier);
        return (
          <View key={tier} accessibilityLabel={`${sizes[tier]}u`} style={styles.flyoutItem}>
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

function ColorSwatchButton({ cube, sizes }: { cube: PlaceableCube; sizes: Omit<SizeGesture, "tap"> }) {
  const { t } = useTranslation();
  const selectCube = useUiStore.getState().selectCube;
  const selected = useInHand({ tool: ActiveTool.Object, cube });
  const ref = useRef<View | null>(null);

  const handlers = useSizeGesture(ref, {
    tap: () => selectCube(cube),
    open: (swatch) => {
      selectCube(cube);
      sizes.open(swatch);
    },
    move: sizes.move,
    lift: sizes.lift,
    close: sizes.close,
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
  const name = decorName(kind);
  const active = useInHand({ tool: ActiveTool.Decor, kind });

  return (
    <Pressable onPress={() => selectDecor(kind)} accessibilityRole="button" accessibilityLabel={t(`decors.${name}`)} style={styles.slot}>
      <Motion to={{ scale: active ? 1.1 : 1 }} transition={SNAP}>
        <View style={[styles.decorDisc, active ? styles.swatchSelected : styles.swatchIdle]}>
          <Glyph glyph={DECOR_GLYPH[name]} size={ITEM_GLYPH} color={active ? STRIP.glyphOn : STRIP.glyph} />
        </View>
      </Motion>
    </Pressable>
  );
}

function ToolButton({ label, active, onPress, children }: { label: string; active?: boolean; onPress: () => void; children: React.ReactNode }) {
  return (
    <Pressable onPress={onPress} accessibilityRole="button" accessibilityLabel={label} style={styles.slot}>
      {
        /* Always mounted: Android, creating it once lit, drew its ground without
                its corners. */
      }
      <View collapsable={false} style={[styles.held, active && styles.toolActive]}>{children}</View>
    </Pressable>
  );
}

export function ModeBar() {
  const { t } = useTranslation();
  const { ui } = useAppContext();
  const [messageExpanded, setMessageExpanded] = useState(false);
  // A downward drag on the bar puts the keyboard away; swatches only open
  // upward, so the two gestures never compete.
  const putAway = useRef(
    PanResponder.create({
      onStartShouldSetPanResponderCapture: keepKeyboard,
      onMoveShouldSetPanResponderCapture: (_e, s) => s.dy > FLYOUT_AXIS_PX && s.dy > Math.abs(s.dx),
      onPanResponderGrant: () => Keyboard.dismiss(),
    }),
  ).current;
  const activeTool = useUiStore((s) => s.activeTool);
  const setActiveTool = useUiStore.getState().setActiveTool;
  const recentPicks = useUiStore((s) => s.recentPicks);
  const appendChatLine = useUiStore.getState().appendChatLine;
  const setObjectTier = useUiStore.getState().setObjectTier;
  const [messageText, setMessageText] = useState("");
  const [isSending, setIsSending] = useState(false);
  const [barWidth, setBarWidth] = useState<f64>(0);
  const [stripWidth, setStripWidth] = useState<f64>(MODE_BAR_HEIGHT_PX);
  const [flyout, setFlyout] = useState<Flyout | null>(null);
  const sizeCount = useKernel((s) => s.object_sizes.length);
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
          setFlyout({ left: sx + sw / 2 - bx, centerX: sx + sw / 2, foot: by + MODE_BAR_HEIGHT_PX - FLYOUT_FOOT, hover: null });
        });
      });
    },
    move: (x, y) => {
      if (flyout === null) return;
      const hover = tierAt(flyout, sizeCount, x, y);
      if (hover !== flyout.hover) setFlyout({ ...flyout, hover });
    },
    lift: (x, y) => {
      const tier = flyout === null ? null : tierAt(flyout, sizeCount, x, y);
      if (tier !== null) setObjectTier(tier);
      setFlyout(null);
    },
    close: () => setFlyout(null),
  };

  const paletteExpanded = !messageExpanded;
  const openFlyout = paletteExpanded ? flyout : null;
  // The send button reuses the palette pill's slot on purpose.
  const showSendInPaletteSlot = messageExpanded && messageText.trim() !== "";
  const pillColor = showSendInPaletteSlot ? color.send : STRIP.ground;
  const slot = paletteExpanded ? Slot.Empty : showSendInPaletteSlot ? Slot.Send : Slot.Palette;

  const room = Math.max(MODE_BAR_HEIGHT_PX, barWidth - MODE_BAR_HEIGHT_PX - PILL_GAP);
  const messageOpen = Math.min(MESSAGE_OPEN_PX, room);
  const messageWidth = messageExpanded ? messageOpen : MODE_BAR_HEIGHT_PX;
  const paletteWidth = paletteExpanded ? Math.min(stripWidth, room) : MODE_BAR_HEIGHT_PX;
  // Whole pixels, where a page's pill cuts its shadow cleanly.
  const messageX = PixelRatio.roundToNearestPixel((barWidth - (messageWidth + PILL_GAP + paletteWidth)) / 2);
  const paletteX = PixelRatio.roundToNearestPixel(messageX + messageWidth + PILL_GAP);

  return (
    <View ref={bar} {...putAway.panHandlers} onLayout={(e) => setBarWidth(e.nativeEvent.layout.width)} style={styles.bar}>
      {barWidth > 0 && (
        <>
          <Pill
            x={messageX}
            width={messageWidth}
            height={MODE_BAR_HEIGHT_PX}
            span={barWidth}
            fill={color.ground}
            place={BackingPlace.OverMap}
            interactive={!messageExpanded}
            shadow={shadow.lift}
          >
            <Pressable
              onPress={() => setMessageExpanded(true)}
              accessibilityRole="button"
              accessibilityLabel={t("map.mode_bar.message_tooltip")}
              style={styles.messageIcon}
            >
              <Glyph glyph="bubble.left" size={chrome.glyph} color={LABEL.primary} />
            </Pressable>
            <Presence
              show={messageExpanded}
              from={{ opacity: 0, translateX: -10 }}
              exit={{ opacity: 0, translateX: -10 }}
              transition={SNAP}
              style={[styles.messageField, { width: messageOpen - MODE_BAR_HEIGHT_PX - 8 }]}
            >
              <Motion to={{ opacity: isSending ? 0 : 1 }} transition={FADE} style={styles.fieldFill}>
                <TextField
                  ref={inputRef}
                  value={messageText}
                  onChangeText={setMessageText}
                  onSubmit={sendMessage}
                  placeholder={t("map.mode_bar.message_placeholder")}
                  placeholderTextColor={LABEL.placeholder}
                  autoFocus
                  style={styles.messageInput}
                />
              </Motion>
            </Presence>
          </Pill>

          <Pill
            x={paletteX}
            width={paletteWidth}
            height={MODE_BAR_HEIGHT_PX}
            span={barWidth}
            fill={pillColor}
            tint={pillColor}
            place={BackingPlace.OverMap}
            interactive={!paletteExpanded}
            shadow={shadow.lift}
            onContentWidth={setStripWidth}
          >
            <Motion
              to={paletteExpanded ? {} : { opacity: 0, translateX: -10 }}
              transition={SNAP}
              style={[styles.strip, { maxWidth: room }, paletteExpanded ? styles.pass : styles.block]}
            >
              <View style={styles.group}>
                <ToolButton label={t("map.mode_bar.pointer")} active={activeTool === ActiveTool.Pointer} onPress={() => setActiveTool(ActiveTool.Pointer)}>
                  <Glyph glyph="hand.point.up.left" size={chrome.glyph} color={activeTool === ActiveTool.Pointer ? STRIP.glyphOn : STRIP.glyph} />
                </ToolButton>
                <ToolButton label={t("map.mode_bar.eraser")} active={activeTool === ActiveTool.Eraser} onPress={() => setActiveTool(ActiveTool.Eraser)}>
                  <Glyph glyph="eraser" size={chrome.glyph} color={activeTool === ActiveTool.Eraser ? STRIP.glyphOn : STRIP.glyph} />
                </ToolButton>
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
                  pick.tool === ActiveTool.Object
                    ? <ColorSwatchButton key={placeableCubeId(pick.cube)} cube={pick.cube} sizes={sizes} />
                    : <DecorIconButton key={`decor:${pick.kind}`} kind={pick.kind} />
                )}
              </ScrollView>

              <View style={styles.divider} />

              <View style={styles.group}>
                <ToolButton label={t("map.mode_bar.catalog_tooltip")} onPress={ui.openDrawerModal}>
                  <Glyph glyph="square.grid.2x2" size={chrome.glyph} color={STRIP.glyph} />
                </ToolButton>
              </View>
            </Motion>

            <Swap
              id={slot}
              from={slot === Slot.Send ? { opacity: 0, scale: SEND_FROM_SCALE } : { opacity: 0 }}
              exit={{ opacity: 0 }}
              transition={slot === Slot.Send ? SNAP : LEAVE}
              exitTransition={LEAVE}
              style={[styles.slotButton, { pointerEvents: paletteExpanded ? "none" : "auto" }]}
            >
              {slot === Slot.Empty ? null : slot === Slot.Send
                ? (
                  <Pressable
                    onPress={sendMessage}
                    disabled={isSending}
                    accessibilityRole="button"
                    accessibilityLabel={t("map.mode_bar.send")}
                    style={({ pressed }) => [styles.slotButton, pressed && styles.sendPressed]}
                  >
                    <Glyph glyph="arrow.up" size={chrome.glyph} weight="bold" color={color.white} />
                  </Pressable>
                )
                : (
                  <Pressable
                    // Not blurred here: blurring on press snapped the pills to the palette on a
                    // device instead of animating.
                    onPress={() => setMessageExpanded(false)}
                    accessibilityRole="button"
                    accessibilityLabel={t("map.mode_bar.palette_tooltip")}
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

const PILL = { height: MODE_BAR_HEIGHT_PX, borderRadius: MODE_BAR_HEIGHT_PX / 2 };

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
  bar: { height: MODE_BAR_HEIGHT_PX, width: "100%", minWidth: 0, pointerEvents: "box-none" },

  messageIcon: { width: MODE_BAR_HEIGHT_PX, height: MODE_BAR_HEIGHT_PX, alignItems: "center", justifyContent: "center" },
  messageField: { flexDirection: "row", alignItems: "center", minWidth: 0 },
  fieldFill: { flex: 1 },
  messageInput: { width: "100%", color: LABEL.primary },

  slotButton: { ...PILL, position: "absolute", left: 0, top: 0, width: MODE_BAR_HEIGHT_PX, alignItems: "center", justifyContent: "center" },
  sendPressed: { opacity: 0.7 },

  strip: { ...PILL, flexDirection: "row", alignItems: "center" },
  // An inline `box-none` is drawn as a `none` the strip's tools inherit.
  pass: { pointerEvents: "box-none" },
  block: { pointerEvents: "none" },
  group: { flexDirection: "row", alignItems: "center", padding: (MODE_BAR_HEIGHT_PX - PITCH) / 2, flexShrink: 0 },
  divider: { width: 1, height: 28, backgroundColor: STRIP.line, flexShrink: 0 },
  picks: { flexShrink: 1, minWidth: 0 },
  picksContent: { alignItems: "center", paddingHorizontal: (MODE_BAR_HEIGHT_PX - PITCH) / 2 },

  slot: { width: PITCH, height: PITCH, alignItems: "center", justifyContent: "center", flexShrink: 0 },
  swatchSlot: PAN_X,
  held: { width: HELD_PX, height: HELD_PX, borderRadius: HELD_PX / 2, alignItems: "center", justifyContent: "center" },
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
  flyoutItem: { width: FLYOUT_ITEM_PX, height: FLYOUT_ITEM_PX, alignItems: "center", justifyContent: "center" },
  markIdle: { backgroundColor: alpha(STRIP.mark, 0.4) },
  markHeld: { backgroundColor: alpha(STRIP.mark, 0.8) },
  markHover: { backgroundColor: STRIP.mark, boxShadow: `0 0 0 1px ${STRIP.mark}` },
});
