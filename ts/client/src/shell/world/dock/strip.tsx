// @pxl/client/shell/world/dock/strip.tsx

/**
 * The palette's card: the tools, the recent picks and the drawer, each in a
 * slot. A swatch's tap takes its cube, and its hold opens the sizes
 * (popover.tsx).
 */

import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import {
  PanResponder,
  Platform,
  Pressable,
  ScrollView,
  StyleSheet,
  View,
  type ViewStyle,
} from "react-native";

import type { f64, u8 } from "@xpute/kit/abi/word.ts";
import type { GlyphName } from "@/shell/ui/icon.tsx";
import { Glyph } from "@/shell/ui/glyph.tsx";
import { Motion } from "@/shell/ui/motion/motion.tsx";
import { SNAP } from "@/shell/ui/motion/transition.ts";
import { alpha, chrome, color } from "@/shell/ui/theme.ts";
import { openModal } from "@/shell/modal.tsx";
import {
  selectTool,
  selectAsset,
  useTool,
  useInHand,
  useRecentPicks,
  useSwatchFlash,
} from "./store.ts";
import { Tool } from "@/abi/cmd.spec.ts";
import { KIND } from "@/shell/catalog/asset/id.spec.ts";
import {
  type AssetId,
  type Cube,
  cubeLabel,
  decorLabel,
  kindOf,
  MARKER,
} from "@/shell/catalog/asset/mod.ts";
import { PIN_LABEL } from "@/shell/catalog/asset/marker.spec.ts";
import {
  CubeSwatch,
  decorGlyph,
  selectionRing,
} from "@/shell/catalog/asset/swatch.tsx";

/** A page's browser pans the strip, and holds and drags up reach the view's own gesture; on a device the responder system shares the finger. */
const PAN_X: ViewStyle =
  Platform.OS === "web" ? ({ touchAction: "pan-x" } as ViewStyle) : {};

const ITEM = 24;

/** How long a press is held to be a long press (React Native `Pressable`'s `delayLongPress`). */
const LONG_PRESS_MS = 300;
/** A move this far before the hold is a drag: the strip's sideways, the deck's up and down. */
export const DRAG_PX = 12;

const PENDING = 0;
const POPOVER = 1;
const DRAG = 2;
type Gesture = typeof PENDING | typeof POPOVER | typeof DRAG;

/** A swatch's gesture. `press`, `pressIn` and `longPress` are React Native
 * `Pressable`'s; `release` is a lift once the sizes are open, and `end` closes
 * them on any lift, a drag or a terminate. */
export interface SizeGesture {
  press: () => void;
  pressIn: (swatch: View) => void;
  /** A key picked the cube or stepped its size: the seed or the sizes over the swatch a moment, as a press shows them. */
  flash: (swatch: View, open: boolean) => void;
  longPress: () => void;
  move: (x: f64, y: f64) => void;
  release: (x: f64, y: f64) => void;
  end: () => void;
}

/** A tap takes the cube; a hold opens the sizes. */
function useSizeGesture(swatch: React.RefObject<View | null>, g: SizeGesture) {
  const handlers = useRef(g);
  handlers.current = g;
  const responder = useRef<ReturnType<typeof PanResponder.create> | null>(null);
  if (!responder.current) {
    let state: Gesture = PENDING;
    let hold: ReturnType<typeof setTimeout> | null = null;
    const stopHold = () => {
      if (hold !== null) clearTimeout(hold);
      hold = null;
    };
    const longPress = () => {
      stopHold();
      if (state !== PENDING) return;
      state = POPOVER;
      handlers.current.longPress();
    };
    responder.current = PanResponder.create({
      onStartShouldSetPanResponder: () => true,
      onPanResponderGrant: () => {
        state = PENDING;
        if (swatch.current) handlers.current.pressIn(swatch.current);
        hold = setTimeout(longPress, LONG_PRESS_MS);
      },
      onPanResponderMove: (_e, s) => {
        if (state === POPOVER) {
          return handlers.current.move(s.moveX, s.moveY);
        }
        if (
          state === DRAG ||
          (Math.abs(s.dx) < DRAG_PX && Math.abs(s.dy) < DRAG_PX)
        )
          return;
        state = DRAG;
        stopHold();
        handlers.current.end();
      },
      onPanResponderRelease: (_e, s) => {
        stopHold();
        if (state === POPOVER) {
          handlers.current.release(s.moveX, s.moveY);
        } else if (state === PENDING) handlers.current.press();
        handlers.current.end();
        state = PENDING;
      },
      onPanResponderTerminate: () => {
        stopHold();
        handlers.current.end();
        state = PENDING;
      },
      onPanResponderTerminationRequest: () => state !== POPOVER,
    });
  }
  return responder.current.panHandlers;
}

function SwatchButton({
  cube,
  gesture,
}: {
  cube: Cube;
  gesture: Omit<SizeGesture, "press">;
}) {
  const selected = useInHand(cube);
  const ref = useRef<View | null>(null);
  const swatchFlash = useSwatchFlash();
  useEffect(() => {
    if (swatchFlash.cube === cube && ref.current) {
      gesture.flash(ref.current, swatchFlash.open);
    }
  }, [swatchFlash]);

  const handlers = useSizeGesture(ref, {
    ...gesture,
    press: () => selectAsset(cube),
    longPress: () => {
      selectAsset(cube);
      gesture.longPress();
    },
  });

  return (
    <View
      ref={ref}
      {...handlers}
      accessibilityRole="button"
      accessibilityLabel={cubeLabel(cube)}
      style={[styles.slot, styles.swatchSlot]}
    >
      <Motion to={{ scale: selected ? 1.1 : 1 }} transition={SNAP}>
        <CubeSwatch
          cube={cube}
          size={ITEM}
          radius={ITEM / 2}
          style={selected ? styles.swatchSelected : styles.swatchIdle}
        />
      </Motion>
    </View>
  );
}

function DecorIconButton({ kind }: { kind: u8 }) {
  const active = useInHand(kind);

  return (
    <Pressable
      onPress={() => selectAsset(kind)}
      accessibilityRole="button"
      accessibilityLabel={decorLabel(kind)}
      style={styles.slot}
    >
      <Motion to={{ scale: active ? 1.1 : 1 }} transition={SNAP}>
        <View
          style={[
            styles.decorDisc,
            active ? styles.swatchSelected : styles.swatchIdle,
          ]}
        >
          <Glyph
            glyph={decorGlyph(kind)}
            size={16}
            color={active ? STRIP.glyphOn : STRIP.glyph}
          />
        </View>
      </Motion>
    </Pressable>
  );
}

/** A hotbar slot: the button of what it holds. */
function PickButton({
  id,
  gesture,
}: {
  id: AssetId;
  gesture: Omit<SizeGesture, "press">;
}) {
  switch (kindOf(id)) {
    case KIND.decor:
      return <DecorIconButton kind={id} />;
    case KIND.marker:
      return <MarkerButton />;
    default:
      return <SwatchButton cube={id} gesture={gesture} />;
  }
}

function MarkerButton() {
  const active = useInHand(MARKER);

  return (
    <Pressable
      onPress={() => selectAsset(MARKER)}
      accessibilityRole="button"
      accessibilityLabel={PIN_LABEL}
      style={styles.slot}
    >
      <Motion to={{ scale: active ? 1.1 : 1 }} transition={SNAP}>
        <View
          style={[
            styles.decorDisc,
            active ? styles.swatchSelected : styles.swatchIdle,
          ]}
        >
          <Glyph
            glyph="bubble.left"
            size={16}
            color={active ? STRIP.glyphOn : STRIP.glyph}
          />
        </View>
      </Motion>
    </Pressable>
  );
}

function ToolButton({
  label,
  glyph,
  active,
  onPress,
}: {
  label: string;
  glyph: GlyphName;
  active?: boolean;
  onPress: () => void;
}) {
  return (
    <Pressable
      onPress={onPress}
      accessibilityRole="button"
      accessibilityLabel={label}
      style={styles.slot}
    >
      {/* Always mounted: Android, creating it once lit, drew its ground without
                its corners. */}
      <View
        collapsable={false}
        style={[styles.held, active && styles.toolActive]}
      >
        <Glyph
          glyph={glyph}
          size={chrome.glyph}
          color={active ? STRIP.glyphOn : STRIP.glyph}
        />
      </View>
    </Pressable>
  );
}

/** `span` is the bar's width, past which it never reaches; the picks scroll unless the sizes are open. */
export function Strip({
  span,
  scrolls,
  gesture,
}: {
  span: f64;
  scrolls: boolean;
  gesture: Omit<SizeGesture, "press">;
}) {
  const { t } = useTranslation();
  const activeTool = useTool();
  const recentPicks = useRecentPicks();

  return (
    <View style={[styles.strip, { maxWidth: span }]}>
      <View style={styles.group}>
        <ToolButton
          label={t("world.dock.pointer")}
          glyph="hand.point.up.left"
          active={activeTool === Tool.Pointer}
          onPress={() => selectTool(Tool.Pointer)}
        />
        <ToolButton
          label={t("world.dock.eraser")}
          glyph="eraser"
          active={activeTool === Tool.Eraser}
          onPress={() => selectTool(Tool.Eraser)}
        />
      </View>

      <View style={styles.divider} />

      <ScrollView
        horizontal
        scrollEnabled={scrolls}
        showsHorizontalScrollIndicator={false}
        style={styles.picks}
        contentContainerStyle={styles.picksContent}
      >
        {recentPicks.map((id) => (
          <PickButton key={id} id={id} gesture={gesture} />
        ))}
      </ScrollView>

      <View style={styles.divider} />

      <View style={styles.group}>
        <ToolButton
          label={t("world.dock.drawer_tooltip")}
          glyph="archivebox"
          onPress={() => openModal("drawer")}
        />
      </View>
    </View>
  );
}

export const STRIP = {
  glyph: alpha(color.white, 0.6),
  glyphOn: color.white,
  mark: color.white,
  held: alpha(color.white, 0.2),
  line: alpha(color.white, 0.15),
  swatchLine: alpha(color.white, 0.3),
  ground: color.band,
};

/** No wider than a target: past it, the later neighbor in the tree takes the tap. */
export const SLOT: ViewStyle = {
  width: chrome.target,
  height: chrome.target,
  alignItems: "center",
  justifyContent: "center",
  flexShrink: 0,
};

const styles = StyleSheet.create({
  strip: {
    height: chrome.control,
    borderRadius: chrome.control / 2,
    flexDirection: "row",
    alignItems: "center",
  },
  group: {
    flexDirection: "row",
    alignItems: "center",
    padding: (chrome.control - chrome.target) / 2,
    flexShrink: 0,
  },
  divider: { width: 1, height: 28, backgroundColor: STRIP.line, flexShrink: 0 },
  picks: { flexShrink: 1, minWidth: 0 },
  picksContent: {
    alignItems: "center",
    paddingHorizontal: (chrome.control - chrome.target) / 2,
  },

  slot: SLOT,
  swatchSlot: PAN_X,
  held: {
    width: chrome.selection,
    height: chrome.selection,
    borderRadius: chrome.selection / 2,
    alignItems: "center",
    justifyContent: "center",
  },
  toolActive: { backgroundColor: STRIP.held },
  decorDisc: {
    width: ITEM,
    height: ITEM,
    borderRadius: ITEM / 2,
    alignItems: "center",
    justifyContent: "center",
    backgroundColor: STRIP.held,
  },
  swatchIdle: { boxShadow: `0 0 0 1px ${STRIP.swatchLine}` },
  swatchSelected: { boxShadow: selectionRing(STRIP.ground, STRIP.mark, 1) },
});
