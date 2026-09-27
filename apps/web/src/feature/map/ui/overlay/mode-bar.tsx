// @pxl/web/feature/map/ui/overlay/mode-bar.tsx

import { useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Keyboard, PanResponder, PixelRatio, Pressable, ScrollView, StyleSheet, type TextInput, View } from "react-native";
import { PAN_X } from "@platform/gesture.ts";

import type { f64, u32, u8 } from "@xpute/core/abi/word.ts";
import { Glass, SYSTEM_GLASS } from "@platform/glass.tsx";
import type { Glyph as GlyphName } from "@platform/icon.tsx";
import { GlassVariant } from "@/shared/ui/glass-variant.ts";
import { Glyph } from "@/shared/ui/glyph.tsx";
import { Motion } from "@platform/motion.tsx";
import { LABEL } from "@platform/palette.ts";
import { Pill } from "@platform/pill.tsx";
import { Presence, Swap } from "@/shared/ui/motion/presence.tsx";
import { LEAVE, SNAP } from "@/shared/ui/motion/transition.ts";
import { keepKeyboard } from "@/shared/ui/keyboard.tsx";
import { TextField } from "@/shared/ui/text-field.tsx";
import { alpha, color, strip } from "@/shared/ui/theme.ts";
import { useAppContext } from "@/app/provider/context.ts";
import { useUiStore } from "@/app/store/ui-store.ts";
import { ActiveTool } from "@/kernel/abi/cmd.spec.ts";
import { decorName, type PlaceableCube, placeableCubeChannel, placeableCubeId, sameCube } from "@/catalog/cell/mod.ts";
import type { DecorId } from "@/catalog/cell/decor.spec.ts";
import { useKernel } from "@/kernel/shell.ts";
import { CubeSwatch } from "./cube-swatch.tsx";

const SEND_FADE_MS = 200;
/** How small the send starts as it grows into its circle. */
const SEND_FROM_SCALE = 0.5;

// The height of both pills — the map view (view.tsx) stands the toasts
// clear of it.
export const MODE_BAR_HEIGHT_PX = 48;
/** A tool or swatch's pitch in the strip. A tap target is as wide as the
 * pitch and no wider — past it, it overlaps a neighbor and the later
 * one in the tree takes the tap — so a swatch smaller than the pitch is a
 * transparent box at full pitch with its shape centered. */
const PITCH = 38;
const GLYPH = 20;
const PILL_GAP = 10;
/** The message field's width where the bar leaves it room. */
const FIELD_PX = 384;
/** The message pill open: its glyph's slot, the field, and room after it. */
const MESSAGE_OPEN_PX = MODE_BAR_HEIGHT_PX + FIELD_PX + 16;

/** How far above the bar the size flyout's bottom stands. */
const FLYOUT_LIFT_PX = 20;
const FLYOUT_ITEM_PX = 40;
const FLYOUT_GAP_PX = 6;
const FLYOUT_PAD_Y = 8;
const FLYOUT_WIDTH = FLYOUT_ITEM_PX + 12;
/** A press held this long on a swatch opens its sizes. */
const FLYOUT_HOLD_MS = 150;
/** A finger that moves this far first decides the gesture: up opens the
 * sizes, sideways leaves the strip to scroll. */
const FLYOUT_AXIS_PX = 12;

/** Where a finger on a swatch is: not yet decided, holding the size flyout
 * open, or given to the strip to scroll. */
export const enum Gesture {
  Pending = 0,
  Flyout = 1,
  Scroll = 2,
}

/** What the palette pill's slot holds while the message pill is open: the
 * way back to the palette, or the send button; nothing while the strip is
 * open over it. */
export const enum Slot {
  Empty = 0,
  Palette = 1,
  Send = 2,
}

/** A size tier's mark in the flyout, a dot, finest the smallest. */
function sizeMark(tier: u32): number {
  return tier === 0 ? 24 : tier === 1 ? 16 : tier === 2 ? 8 : 4;
}

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
  const object_tier = useUiStore((s) => s.object_tier);
  const sizes = useKernel((s) => s.object_sizes);

  return (
    <Glass
      fill={STRIP.flyout}
      variant={GlassVariant.Regular}
      fade={shown ? 1 : 0}
      fadeFrom={0}
      transition={shown ? SNAP : LEAVE}
      style={styles.flyout}
    >
      {sizeTiers(sizes.length).map((tier) => {
        const mark = sizeMark(tier);
        return (
          <View key={tier} accessibilityLabel={`${sizes[tier]}u`} style={styles.flyoutItem}>
            <Motion to={{ scale: hover === tier ? 1.25 : 1 }} transition={SNAP}>
              <View
                style={[
                  { width: mark, height: mark, borderRadius: mark / 2 },
                  hover === tier ? styles.markHover : object_tier === tier ? styles.markHeld : styles.markIdle,
                ]}
              />
            </Motion>
          </View>
        );
      })}
    </Glass>
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
  const activeTool = useUiStore((s) => s.activeTool);
  const held = useUiStore((s) => s.cube);
  const selectCube = useUiStore.getState().selectCube;
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

  const selected = activeTool === ActiveTool.Object && sameCube(held, cube);

  return (
    <View ref={ref} {...handlers} accessibilityRole="button" accessibilityLabel={t(`cubes.${placeableCubeId(cube)}`)} style={[styles.slot, styles.swatchSlot]}>
      <Motion to={{ scale: selected ? 1.1 : 1 }} transition={SNAP}>
        <CubeSwatch cube={cube} size={GLYPH} radius={GLYPH / 2} style={selected ? styles.swatchSelected : styles.swatchIdle} />
      </Motion>
    </View>
  );
}

// The hotbar's decor slot(s): recentPicks (ui-store.ts) is one MRU list
// across cube and decor. Unlike a
// ColorSwatchButton, a decor pick has no size flyout of its own (DECOR_SIZE
// is fixed, pxl-kernel layer/object/size.rs), so this stays a plain tap button.
function DecorIconButton({ kind }: { kind: u8 }) {
  const { t } = useTranslation();
  const activeTool = useUiStore((s) => s.activeTool);
  const held = useUiStore((s) => s.decor);
  const selectDecor = useUiStore.getState().selectDecor;
  const name = decorName(kind);
  const active = activeTool === ActiveTool.Decor && held === kind;

  return (
    <ToolButton label={t(`decors.${name}`)} active={active} onPress={() => selectDecor(kind)}>
      <Glyph glyph={DECOR_ICONS[name]} size={GLYPH} color={active ? STRIP.glyphOn : STRIP.glyph} />
    </ToolButton>
  );
}

// Icon per decor kind, by name — TypeScript enforces one entry per DecorId, so
// adding a new decor kind (decor.spec.ts's own decor_types) can't silently ship
// without a button icon for it. Lives here (this file's own DecorIconButton
// is one consumer) and is also imported by drawer-modal.tsx's own
// DecorGridButton, so a new decor kind's icon shows up in both the hotbar
// and the drawer for free.
export const DECOR_ICONS: Record<DecorId, GlyphName> = {
  tree: "tree",
};

/** A round slot in the dark strip, lit while its tool is the one in hand. */
function ToolButton({ label, active, onPress, children }: { label: string; active?: boolean; onPress: () => void; children: React.ReactNode }) {
  return (
    <Pressable onPress={onPress} accessibilityRole="button" accessibilityLabel={label} style={[styles.slot, styles.round, active && styles.toolActive]}>
      {children}
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
  const append_chat_line = useUiStore.getState().append_chat_line;
  const setObjectTier = useUiStore.getState().setObjectTier;
  const sizeCount = useKernel((s) => s.object_sizes.length);
  const [messageText, setMessageText] = useState("");
  const [isSending, setIsSending] = useState(false);
  const [barWidth, setBarWidth] = useState<f64>(0);
  const [stripWidth, setStripWidth] = useState<f64>(MODE_BAR_HEIGHT_PX);
  const [flyoutAt, setFlyoutAt] = useState<FlyoutAt | null>(null);
  const [hover, setHover] = useState<u32 | null>(null);
  const inputRef = useRef<TextInput | null>(null);
  const bar = useRef<View | null>(null);
  // The flyout's last place, which it leaves from after the gesture is over.
  const flyoutLeft = useRef<f64>(0);
  if (flyoutAt) flyoutLeft.current = flyoutAt.left;

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

  const sizes: Omit<SizeGesture, "tap"> = {
    open: (swatch) => {
      swatch.measureInWindow((sx, _sy, sw) => {
        bar.current?.measureInWindow((bx, by) => setFlyoutAt({ left: sx + sw / 2 - bx, centerX: sx + sw / 2, barTop: by }));
      });
      setHover(null);
    },
    move: (x, y) => {
      if (!flyoutAt) return;
      const tier = tierAt(flyoutAt, sizeCount, x, y);
      if (tier !== hover) setHover(tier);
    },
    lift: (x, y) => {
      const tier = flyoutAt && tierAt(flyoutAt, sizeCount, x, y);
      if (tier !== null) setObjectTier(tier);
      setFlyoutAt(null);
      setHover(null);
    },
    close: () => {
      setFlyoutAt(null);
      setHover(null);
    },
  };

  const paletteExpanded = !messageExpanded;
  const flyoutOpen = flyoutAt !== null && paletteExpanded;
  // Reuses the palette pill's own idle collapsed-icon slot as the send
  // button rather than adding a separate one inside the message pill — the
  // same idle-slot-reuse pattern as the size flyout anchoring per-swatch
  // instead of to one shared control. Reviewed and kept on purpose: a
  // rework does not drop it without raising it.
  const showSendInPaletteSlot = messageExpanded && messageText.trim() !== "";
  // The send button is the palette pill's glass tinted with the send color;
  // without glass, the same color is the fill.
  const sendTint = showSendInPaletteSlot ? color.send : undefined;

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
            fill={color.white}
            variant={GlassVariant.Regular}
            interactive={!messageExpanded}
            shadow="0 6px 20px rgba(0, 0, 0, 0.25)"
          >
            <Pressable
              onPress={() => setMessageExpanded(true)}
              accessibilityRole="button"
              accessibilityLabel={t("map.mode_bar.message_tooltip")}
              style={styles.messageIcon}
            >
              <Glyph glyph="bubble.left" size={GLYPH} color={LABEL.primary} />
            </Pressable>
            <Presence
              show={messageExpanded}
              from={{ opacity: 0, translateX: -10 }}
              exit={{ opacity: 0, translateX: -10 }}
              transition={SNAP}
              style={[styles.messageField, { width: messageOpen - MODE_BAR_HEIGHT_PX - 8 }]}
            >
              <Motion to={{ opacity: isSending ? 0 : 1 }} transition={{ ...LEAVE, durationMs: SEND_FADE_MS }} style={styles.fieldFill}>
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
            fill={sendTint ?? STRIP.ground}
            tint={sendTint}
            variant={GlassVariant.Regular}
            interactive={!paletteExpanded}
            shadow="0 3px 6px rgba(0, 0, 0, 0.25)"
            onContentWidth={setStripWidth}
          >
            <Motion
              to={paletteExpanded ? {} : { opacity: 0, translateX: -10 }}
              transition={SNAP}
              style={[styles.strip, { maxWidth: room }, paletteExpanded ? styles.pass : styles.block]}
            >
              <View style={styles.group}>
                <ToolButton label={t("map.mode_bar.pointer")} active={activeTool === ActiveTool.Pointer} onPress={() => set_activeTool(ActiveTool.Pointer)}>
                  <Glyph glyph="hand.point.up.left" size={GLYPH} color={activeTool === ActiveTool.Pointer ? STRIP.glyphOn : STRIP.glyph} />
                </ToolButton>
                <ToolButton label={t("map.mode_bar.eraser")} active={activeTool === ActiveTool.Eraser} onPress={() => set_activeTool(ActiveTool.Eraser)}>
                  <Glyph glyph="eraser" size={GLYPH} color={activeTool === ActiveTool.Eraser ? STRIP.glyphOn : STRIP.glyph} />
                </ToolButton>
              </View>

              <View style={styles.divider} />

              <ScrollView
                horizontal
                scrollEnabled={flyoutAt === null}
                showsHorizontalScrollIndicator={false}
                style={styles.picks}
                contentContainerStyle={styles.picksContent}
              >
                {recentPicks.map((pick) =>
                  pick.tool === ActiveTool.Object
                    ? <ColorSwatchButton key={`${placeableCubeChannel(pick.cube)}:${pick.cube.type}`} cube={pick.cube} sizes={sizes} />
                    : <DecorIconButton key={`decor:${pick.kind}`} kind={pick.kind} />
                )}
              </ScrollView>

              <View style={styles.divider} />

              {
                // Opens the full catalog: every terrain-styled block
                // type and decor kind that hasn't (yet) earned a hotbar
                // slot, plus Character — Minecraft's own hotbar/
                // inventory relationship. Outside the scrolling strip
                // above so it's always reachable regardless of scroll
                // position.
              }
              <View style={styles.group}>
                <ToolButton label={t("map.mode_bar.catalog_tooltip")} onPress={ui.openDrawerModal}>
                  <Glyph glyph="square.grid.2x2" size={GLYPH} color={STRIP.glyph} />
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
              id={paletteExpanded ? Slot.Empty : showSendInPaletteSlot ? Slot.Send : Slot.Palette}
              from={showSendInPaletteSlot ? { opacity: 0, scale: SEND_FROM_SCALE } : { opacity: 0 }}
              exit={{ opacity: 0 }}
              transition={showSendInPaletteSlot ? SNAP : LEAVE}
              exitTransition={LEAVE}
              style={[styles.slotButton, { pointerEvents: paletteExpanded ? "none" : "auto" }]}
            >
              {paletteExpanded ? null : showSendInPaletteSlot
                ? (
                  <Pressable
                    onPress={sendMessage}
                    disabled={isSending}
                    accessibilityRole="button"
                    accessibilityLabel={t("map.mode_bar.send")}
                    style={({ pressed }) => [styles.slotButton, pressed && styles.sendPressed]}
                  >
                    <Glyph glyph="arrow.up" size={GLYPH} weight="bold" color={color.white} />
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
                    <Glyph glyph="paintpalette" size={GLYPH} color={STRIP.glyph} />
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
            style={[styles.flyoutAt, { left: flyoutLeft.current - FLYOUT_WIDTH / 2 }]}
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
 * What the strip and the size flyout draw in. Where they are a fill — the
 * page, a device without the system's glass — a dark band with white glyphs.
 * On the system's glass, a toolbar as the platform draws one over light
 * content: the glass untinted, its glyphs the label color as every
 * control's are, the tool in hand on a faint capsule. Glass painted dark to
 * pass for the band only clouded it: a tint
 * is for what a surface means, and the glass decides its own light and dark
 * from what is under it.
 */
const STRIP = SYSTEM_GLASS ? strip.glass : strip.band;

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
  group: { flexDirection: "row", alignItems: "center", padding: 4, flexShrink: 0 },
  divider: { width: 1, height: 28, backgroundColor: STRIP.line, flexShrink: 0 },
  picks: { flexShrink: 1, minWidth: 0 },
  picksContent: { alignItems: "center", paddingHorizontal: 4 },

  slot: { width: PITCH, height: PITCH, alignItems: "center", justifyContent: "center", flexShrink: 0 },
  swatchSlot: PAN_X,
  round: { borderRadius: PITCH / 2 },
  toolActive: { backgroundColor: STRIP.held },
  swatchIdle: { boxShadow: `0 0 0 1px ${STRIP.line}` },
  swatchSelected: { boxShadow: `0 0 0 1px ${STRIP.ground}, 0 0 0 2px ${STRIP.ink}` },

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
    boxShadow: "0 6px 20px rgba(0, 0, 0, 0.25)",
  },
  flyoutItem: { width: FLYOUT_ITEM_PX, height: FLYOUT_ITEM_PX, alignItems: "center", justifyContent: "center" },
  markIdle: { backgroundColor: alpha(STRIP.ink, 0.4) },
  markHeld: { backgroundColor: alpha(STRIP.ink, 0.8) },
  markHover: { backgroundColor: STRIP.ink, boxShadow: `0 0 0 1px ${STRIP.ink}` },
});
