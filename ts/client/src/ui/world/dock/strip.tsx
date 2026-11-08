// @pxl/client/ui/world/dock/strip.tsx

/**
 * The palette's card: the tools, the hotbar and the drawer's button, each in
 * a slot. A tap on a cube's swatch puts the cube in hand, and a hold opens its
 * sizes (popover.tsx).
 */

import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import {
  PanResponder,
  Pressable,
  ScrollView,
  StyleSheet,
  View,
  type ViewStyle,
} from "react-native";

import type { f64, u8 } from "@xpute/kit/abi/word.ts";
import type { GlyphName } from "@/ui/component/icon.tsx";
import { Glyph } from "@/ui/component/glyph.tsx";
import { Motion } from "@/ui/component/motion/motion.tsx";
import { SNAP } from "@/ui/component/motion/transition.ts";
import { alpha, chrome, color } from "@/ui/component/theme.ts";
import { panX } from "@/ui/component/pan.ts";
import { openModal } from "@/ui/modal.tsx";
import {
  selectTool,
  selectAsset,
  useTool,
  useInHand,
  useRecentPicks,
  useSwatchFlash,
} from "./store.ts";
import { Tool } from "@/abi/cmd.spec.ts";
import { KIND } from "@/ui/asset/id.spec.ts";
import {
  type AssetId,
  type Cube,
  cubeLabel,
  decorLabel,
  kindOf,
  MARKER,
} from "@/ui/asset/catalog.ts";
import { PIN_LABEL } from "@/ui/asset/marker.spec.ts";
import { CubeSwatch, decorGlyph, selectionRing } from "@/ui/asset/swatch.tsx";

const ITEM = 24;

/** How long a press lasts before it counts as a hold, as `Pressable`'s
 * `delayLongPress` would. */
const LONG_PRESS_MS = 300;
/** A finger that moves this far before the hold time is a drag, not a press.
 * Sideways it scrolls the strip; up or down it turns the deck. */
export const DRAG_PX = 12;

const PENDING = 0;
const POPOVER = 1;
const DRAG = 2;
type Gesture = typeof PENDING | typeof POPOVER | typeof DRAG;

/** The handlers of a swatch's gesture. `press`, `pressIn` and `longPress` mean
 * what they mean on React Native's `Pressable`. `release` is a lift while the
 * sizes are open. `end` closes the sizes; it comes on every lift, on a drag,
 * and when the gesture is taken away. */
export interface SizeGesture {
  press: () => void;
  pressIn: (swatch: View) => void;
  /** A key picked the cube or stepped its size. Shows the seed, or the sizes
   * when `open`, over the swatch for a moment, as a press would. */
  flash: (swatch: View, open: boolean) => void;
  longPress: () => void;
  move: (x: f64, y: f64) => void;
  release: (x: f64, y: f64) => void;
  end: () => void;
}

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
      {/* The highlight stays mounted and only its color changes. On Android,
          a view created with its background already set is drawn without its
          rounded corners. */}
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

/** `span` is the bar's width, and the strip never grows past it. The hotbar
 * does not scroll while the sizes are open. */
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

/** A slot is one touch target wide. A wider slot would overlap its neighbor,
 * and of two overlapping views the later one in the tree takes the tap. */
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
  swatchSlot: panX,
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
