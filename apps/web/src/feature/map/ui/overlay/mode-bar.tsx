// @pxl/web/feature/map/ui/overlay/mode-bar.tsx

import { useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Keyboard, PanResponder, PixelRatio, Pressable, ScrollView, StyleSheet, type TextInput, View } from "react-native";
import { PAN_X } from "@platform/gesture.ts";

import type { f64, u32, u8 } from "@xpute/core/abi/word.ts";
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
import { alpha, chrome, color, shadow } from "@/shared/ui/theme.ts";
import { useAppContext } from "@/app/provider/context.ts";
import { useInHand, useUiStore } from "@/app/store/ui-store.ts";
import { ActiveTool } from "@/kernel/abi/cmd.spec.ts";
import { decorName, type PlaceableCube, placeableCubeId } from "@/catalog/cell/mod.ts";
import { useKernel } from "@/kernel/shell.ts";
import { cubeLabel, CubeSwatch, DECOR_GLYPH, heldRing } from "./cube-swatch.tsx";

/** How small the send starts as it grows into its circle. */
const SEND_FROM_SCALE = 0.5;

// The height of both pills, a round control's side — the map view
// (view.tsx) stands the toasts clear of it.
export const MODE_BAR_HEIGHT_PX = chrome.control;
/** A tool or swatch's pitch in the strip. A tap target is as wide as the
 * pitch and no wider — past it, it overlaps a neighbor and the later
 * one in the tree takes the tap — so a swatch smaller than the pitch is a
 * transparent box at full pitch with its shape centered. */
const PITCH = chrome.target;
/** A pick in the strip, a cube's swatch or a decor's disc: a round item, a
 * step over a tool's glyph, since it is a thing and not an action. */
const ITEM = 24;
/** A decor's glyph on its disc. */
const ITEM_GLYPH = 16;
/** The capsule a tool in hand is lit on, centered in its slot. */
const HELD_PX = chrome.selection;
const PILL_GAP = chrome.controlGap;
/** The message field's width where the bar leaves it room. */
const FIELD_PX = 384;
/** The message pill open: its glyph's slot, the field, and room after it. */
const MESSAGE_OPEN_PX = MODE_BAR_HEIGHT_PX + FIELD_PX + 16;

/** How far above the bar the size flyout's bottom stands. */
const FLYOUT_LIFT_PX = 20;
const FLYOUT_ITEM_PX = 40;
const FLYOUT_GAP_PX = 8;
const FLYOUT_PAD_Y = 8;
const FLYOUT_WIDTH = FLYOUT_ITEM_PX + 12;
/** A press held this long on a swatch opens its sizes. */
const FLYOUT_HOLD_MS = 150;
/** A finger that moves this far first decides the gesture: up opens the
 * sizes, sideways leaves the strip to scroll. */
const FLYOUT_AXIS_PX = 12;

/** Where a finger on a swatch is: not yet decided, holding the size flyout
 * open, or given to the strip to scroll. */
const enum Gesture {
  Pending = 0,
  Flyout = 1,
  Scroll = 2,
}

/** What the palette pill's slot holds while the message pill is open: the
 * way back to the palette, or the send button; nothing while the strip is
 * open over it. */
const enum Slot {
  Empty = 0,
  Palette = 1,
  Send = 2,
}

/** Each size tier's mark in the flyout, a dot, the coarsest largest: one per
 * tier the kernel has, so a fifth tier needs its mark named here. */
const SIZE_MARKS = [24, 16, 8, 4];

/** The tiers top to bottom: coarsest first, as the marks shrink downward. */
function sizeTiers(count: u32): u32[] {
  return Array.from({ length: count }, (_, i) => count - 1 - i);
}

/** Where the size flyout stands, in the bar's frame and in the window's: its
 * center over the swatch, and the bar's top, which its foot stands above. */
interface FlyoutAt {
  left: f64;
  centerX: f64;
  barTop: f64;
}

/** The tier under a point in the window, or null: the flyout's items are a
 * fixed pitch above the bar, so this is arithmetic on the point, and nothing
 * is measured while the finger moves. */
function tierAt(at: FlyoutAt, count: u32, x: f64, y: f64): u32 | null {
  if (Math.abs(x - at.centerX) > FLYOUT_WIDTH / 2) return null;
  const bottom = at.barTop - FLYOUT_LIFT_PX;
  const top = bottom - (2 * FLYOUT_PAD_Y + count * FLYOUT_ITEM_PX + (count - 1) * FLYOUT_GAP_PX);
  const along = y - top - FLYOUT_PAD_Y;
  if (along < 0) return null;
  const row = Math.floor(along / (FLYOUT_ITEM_PX + FLYOUT_GAP_PX));
  if (row >= count || along - row * (FLYOUT_ITEM_PX + FLYOUT_GAP_PX) > FLYOUT_ITEM_PX) return null;
  return sizeTiers(count)[row];
}

/** The size flyout over the swatch it was opened from: the tier the finger
 * is over is marked, and the one it lifts on is the size placed from then
 * on. Drawn from the bar rather than from the swatch, which sits in a
 * horizontally scrolling strip that clips what stands outside it. */
function SizeFlyout({ shown, hover }: { shown: boolean; hover: u32 | null }) {
  const objectTier = useUiStore((s) => s.objectTier);
  const sizes = useKernel((s) => s.object_sizes);

  return (
    <Backing
      fill={STRIP.flyout}
      tint={STRIP.ground}
      place={BackingPlace.OverMap}
      fade={shown ? 1 : 0}
      fadeFrom={0}
      transition={shown ? SNAP : LEAVE}
      style={styles.flyout}
    >
      {sizeTiers(sizes.length).map((tier) => {
        const mark = SIZE_MARKS[tier];
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
  /** The swatch was pressed and let go in place. */
  tap: () => void;
  /** Held, or dragged up: the sizes open over this swatch. */
  open: (swatch: View) => void;
  /** The finger over the open sizes, in the window. */
  move: (x: f64, y: f64) => void;
  /** Lifted over the open sizes, in the window. */
  lift: (x: f64, y: f64) => void;
  /** The sizes close without a pick. */
  close: () => void;
}

/**
 * A swatch's one gesture: a tap takes the cube; a hold, or a drag up, opens
 * the sizes over it, the finger slides to one and lifts to take it. A drag
 * sideways is the strip's to scroll, which takes the touch over from this.
 * The script hears the finger only to decide and to pick — the flyout's
 * arrival and the mark's growth are poses the compositor plays.
 */
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
      // Once the sizes are open the finger is theirs; before that, a strip
      // that starts to scroll may have it.
      onPanResponderTerminationRequest: () => state !== Gesture.Flyout,
    });
  }
  return responder.current.panHandlers;
}

/** A cube in the strip: a tap takes it in hand, a hold or a drag up opens the
 * size flyout over it. */
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

/** A decor kind in the strip, on a disc as a cube's swatch is: a plain tap, since a decor has one size
 * (DECOR_SIZE, pxl-kernel layer/object/size.rs) and so no flyout. */
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

/** A round slot in the strip, lit while its tool is the one in hand. */
function ToolButton({ label, active, onPress, children }: { label: string; active?: boolean; onPress: () => void; children: React.ReactNode }) {
  return (
    <Pressable onPress={onPress} accessibilityRole="button" accessibilityLabel={label} style={styles.slot}>
      {
        /* A view from the start, though unlit it draws nothing: a view with
        nothing to draw is left out of the native tree, and Android, making
        it once it is lit, drew its ground without its corners. */
      }
      <View collapsable={false} style={[styles.held, active && styles.toolActive]}>{children}</View>
    </Pressable>
  );
}

/**
 * The bar: a message pill and a palette pill side by side, one open and the
 * other a circle, centered together. The bar works out where each stands and
 * how wide it shows for the mode, and a change of mode moves them there the
 * way each platform moves a pill (shared/ui/pill.ts).
 */
export function ModeBar() {
  const { t } = useTranslation();
  const { ui } = useAppContext();
  // The message pill open, or the palette strip: the two share the bar.
  const [messageExpanded, setMessageExpanded] = useState(false);
  // The bar is where a message is typed and sent: a touch that starts on its
  // pills leaves the keyboard up (shared/ui/keyboard.tsx). A finger drawn
  // down it puts the keyboard away, as one drawn down a chat's messages does;
  // it decides on the distance a swatch's gesture does, and only downward,
  // where a swatch's opens its sizes upward, so the two never want the same
  // finger.
  const putAway = useRef(
    PanResponder.create({
      onStartShouldSetPanResponderCapture: keepKeyboard,
      onMoveShouldSetPanResponderCapture: (_e, s) => s.dy > FLYOUT_AXIS_PX && s.dy > Math.abs(s.dx),
      onPanResponderGrant: () => Keyboard.dismiss(),
    }),
  ).current;
  const activeTool = useUiStore((s) => s.activeTool);
  const set_activeTool = useUiStore.getState().set_activeTool;
  const recentPicks = useUiStore((s) => s.recentPicks);
  const appendChatLine = useUiStore.getState().appendChatLine;
  const setObjectTier = useUiStore.getState().setObjectTier;
  const sizeCount = useKernel((s) => s.object_sizes.length);
  const [messageText, setMessageText] = useState("");
  const [isSending, setIsSending] = useState(false);
  const [barWidth, setBarWidth] = useState<f64>(0);
  const [stripWidth, setStripWidth] = useState<f64>(MODE_BAR_HEIGHT_PX);
  // Where the size flyout stands is kept after it closes, so it leaves from
  // there.
  const [flyout, setFlyout] = useState<{ at: FlyoutAt; shown: boolean } | null>(null);
  const flyoutShown = flyout?.shown ?? false;
  const hide = () => setFlyout((f) => f && { ...f, shown: false });
  const [hover, setHover] = useState<u32 | null>(null);
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
          setFlyout({ at: { left: sx + sw / 2 - bx, centerX: sx + sw / 2, barTop: by }, shown: true });
        });
      });
      setHover(null);
    },
    move: (x, y) => {
      if (!flyout?.shown) return;
      const tier = tierAt(flyout.at, sizeCount, x, y);
      if (tier !== hover) setHover(tier);
    },
    lift: (x, y) => {
      const tier = flyout?.shown ? tierAt(flyout.at, sizeCount, x, y) : null;
      if (tier !== null) setObjectTier(tier);
      hide();
      setHover(null);
    },
    close: () => {
      hide();
      setHover(null);
    },
  };

  const paletteExpanded = !messageExpanded;
  const flyoutOpen = flyoutShown && paletteExpanded;
  // Reuses the palette pill's own idle collapsed-icon slot as the send
  // button rather than adding a separate one inside the message pill — the
  // same idle-slot-reuse pattern as the size flyout anchoring per-swatch
  // instead of to one shared control. Reviewed and kept on purpose: a
  // rework does not drop it without raising it.
  const showSendInPaletteSlot = messageExpanded && messageText.trim() !== "";
  // The pill is the strip's black, or the send's coral while it is the send
  // button: the tint on the glass, and without glass the fill.
  const pillColor = showSendInPaletteSlot ? color.send : STRIP.ground;
  const slot = paletteExpanded ? Slot.Empty : showSendInPaletteSlot ? Slot.Send : Slot.Palette;

  // Where each pill stands and how wide it shows. The open pill takes what
  // the other's circle and the gap leave, up to its own width; the pair is
  // centered on the bar.
  const room = Math.max(MODE_BAR_HEIGHT_PX, barWidth - MODE_BAR_HEIGHT_PX - PILL_GAP);
  const messageOpen = Math.min(MESSAGE_OPEN_PX, room);
  const messageWidth = messageExpanded ? messageOpen : MODE_BAR_HEIGHT_PX;
  const paletteWidth = paletteExpanded ? Math.min(stripWidth, room) : MODE_BAR_HEIGHT_PX;
  // On whole pixels, where a page's pill cuts its shadow cleanly
  // (platform/web/pill.tsx).
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
                <ToolButton label={t("map.mode_bar.pointer")} active={activeTool === ActiveTool.Pointer} onPress={() => set_activeTool(ActiveTool.Pointer)}>
                  <Glyph glyph="hand.point.up.left" size={chrome.glyph} color={activeTool === ActiveTool.Pointer ? STRIP.glyphOn : STRIP.glyph} />
                </ToolButton>
                <ToolButton label={t("map.mode_bar.eraser")} active={activeTool === ActiveTool.Eraser} onPress={() => set_activeTool(ActiveTool.Eraser)}>
                  <Glyph glyph="eraser" size={chrome.glyph} color={activeTool === ActiveTool.Eraser ? STRIP.glyphOn : STRIP.glyph} />
                </ToolButton>
              </View>

              <View style={styles.divider} />

              <ScrollView
                horizontal
                scrollEnabled={!flyoutShown}
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

              {
                // The whole catalog, as Minecraft's inventory is to its
                // hotbar; outside the scrolling picks, so it is always in
                // reach.
              }
              <View style={styles.group}>
                <ToolButton label={t("map.mode_bar.catalog_tooltip")} onPress={ui.openDrawerModal}>
                  <Glyph glyph="square.grid.2x2" size={chrome.glyph} color={STRIP.glyph} />
                </ToolButton>
              </View>
            </Motion>

            {
              /* The send arrives as a message app's does, growing into its
                circle on a spring the moment there is something to send;
                whatever leaves the slot, and the palette coming back, only
                fade. */
            }
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
                    // The field is not blurred here: it lets go of focus, and the keyboard
                    // goes, when it has slid out and unmounts. Blurred on this press, the
                    // pills snapped to the palette on a device instead of moving there.
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

          <Presence
            show={flyoutOpen}
            from={{ scale: 0.9, translateY: 8 }}
            exit={{ scale: 0.9, translateY: 8 }}
            transition={SNAP}
            exitTransition={LEAVE}
            style={[styles.flyoutAt, { left: (flyout?.at.left ?? 0) - FLYOUT_WIDTH / 2 }]}
          >
            {
              // Faded by its glass, not by the presence around it
              // (shared/ui/motion/veil.ts).
            }
            <SizeFlyout shown={flyoutOpen} hover={hover} />
          </Presence>
        </>
      )}
    </View>
  );
}

const PILL = { height: MODE_BAR_HEIGHT_PX, borderRadius: MODE_BAR_HEIGHT_PX / 2 };

/**
 * What the strip and the size flyout draw in: a black pill with white
 * glyphs on every platform, the tool in hand on a lighter capsule. The strip
 * is a tool rather than the map's chrome, drawn as the send button that takes
 * its slot is — a filled color, the send's coral or the strip's black: the
 * system's glass tinted with it on an iPhone, the color itself as a fill
 * elsewhere. `mark` is the glyph color as a hex, for what is drawn at an
 * opacity of it.
 */
const STRIP = {
  glyph: alpha(color.white, 0.6),
  glyphOn: color.white,
  mark: color.white,
  held: alpha(color.white, 0.2),
  line: alpha(color.white, 0.15),
  ground: color.band,
  flyout: alpha(color.band, 0.8),
};

const styles = StyleSheet.create({
  // As tall as the pills and as wide as the row: the pills stand on it by
  // their poses, not in its flow.
  bar: { height: MODE_BAR_HEIGHT_PX, width: "100%", minWidth: 0, pointerEvents: "box-none" },

  messageIcon: { width: MODE_BAR_HEIGHT_PX, height: MODE_BAR_HEIGHT_PX, alignItems: "center", justifyContent: "center" },
  messageField: { flexDirection: "row", alignItems: "center", minWidth: 0 },
  fieldFill: { flex: 1 },
  messageInput: { width: "100%", color: LABEL.primary },

  slotButton: { ...PILL, position: "absolute", left: 0, top: 0, width: MODE_BAR_HEIGHT_PX, alignItems: "center", justifyContent: "center" },
  sendPressed: { opacity: 0.7 },

  strip: { ...PILL, flexDirection: "row", alignItems: "center" },
  // From the sheet: an inline `box-none` is drawn as a `none` the strip's
  // tools inherit (feature/map/ui/busy-dim.tsx).
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
  swatchIdle: { boxShadow: `0 0 0 1px ${STRIP.line}` },
  swatchSelected: { boxShadow: heldRing(STRIP.ground, STRIP.mark, 1) },

  // Its foot stands over the bar, its scale grows from there.
  flyoutAt: {
    position: "absolute",
    bottom: MODE_BAR_HEIGHT_PX + FLYOUT_LIFT_PX,
    width: FLYOUT_WIDTH,
    zIndex: 10,
    transformOrigin: "center bottom",
    pointerEvents: "none",
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
