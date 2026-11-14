// @pxl/client/ui/feature/world/dock/strip.tsx

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

import type { f64 } from "@xpute/kit/abi/word.ts";
import type { GlyphName } from "@/ui/shared/component/icon.tsx";
import { Glyph } from "@/ui/shared/component/glyph.tsx";
import { Motion } from "@/ui/shared/component/motion/motion.tsx";
import { SNAP } from "@/ui/shared/component/motion/transition.ts";
import { alpha, CHROME, COLOR } from "@/ui/shared/component/theme.ts";
import { PAN_X } from "@/ui/shared/component/pan.ts";
import { open_modal } from "@/ui/shared/modal.ts";
import {
  select_tool,
  select_asset,
  useTool,
  useInHand,
  useRecentPicks,
  useSwatchFlash,
} from "./store.ts";
import { TOOL_ERASER, TOOL_POINTER } from "@/abi/edit.enum.ts";
import { KIND_DECOR, KIND_MARKER } from "@/ui/shared/asset/id.spec.ts";
import {
  type AssetId,
  type Cube,
  cube_label,
  decor_label,
  kind_of,
  MARKER,
} from "@/ui/shared/asset/catalog.ts";
import { PIN_LABEL } from "@/ui/shared/asset/marker.spec.ts";
import {
  CubeSwatch,
  decor_glyph,
  selection_ring,
} from "@/ui/shared/asset/swatch.tsx";

const ITEM = 24;

const LONG_PRESS_MS = 300;
/** A finger that moves this far before the hold time is a drag, not a press.
 * Sideways it scrolls the strip; up or down it turns the deck. */
export const DRAG_PX = 12;

const PENDING = 0;
const POPOVER = 1;
const DRAG = 2;
type Gesture = typeof PENDING | typeof POPOVER | typeof DRAG;

/** The handlers of a swatch's gesture. `press`, `press_in` and `long_press` mean
 * what they mean on React Native's `Pressable`. `release` is a lift while the
 * sizes are open. `end` closes the sizes; it comes on every lift, on a drag,
 * and when the gesture is taken away. */
export interface SizeGesture {
  press: () => void;
  press_in: (swatch: View) => void;
  /** A key picked the cube or stepped its size. Shows the seed, or the sizes
   * when `open`, over the swatch for a moment, as a press would. */
  flash: (swatch: View, open: boolean) => void;
  long_press: () => void;
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
    const stop_hold = () => {
      if (hold !== null) clearTimeout(hold);
      hold = null;
    };
    const long_press = () => {
      stop_hold();
      if (state !== PENDING) return;
      state = POPOVER;
      handlers.current.long_press();
    };
    responder.current = PanResponder.create({
      onStartShouldSetPanResponder: () => true,
      onPanResponderGrant: () => {
        state = PENDING;
        if (swatch.current) handlers.current.press_in(swatch.current);
        hold = setTimeout(long_press, LONG_PRESS_MS);
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
        stop_hold();
        handlers.current.end();
      },
      onPanResponderRelease: (_e, s) => {
        stop_hold();
        if (state === POPOVER) {
          handlers.current.release(s.moveX, s.moveY);
        } else if (state === PENDING) handlers.current.press();
        handlers.current.end();
        state = PENDING;
      },
      onPanResponderTerminate: () => {
        stop_hold();
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
  const swatch_flash = useSwatchFlash();
  useEffect(() => {
    if (swatch_flash.cube === cube && ref.current) {
      gesture.flash(ref.current, swatch_flash.open);
    }
  }, [swatch_flash]);

  const handlers = useSizeGesture(ref, {
    ...gesture,
    press: () => select_asset(cube),
    long_press: () => {
      select_asset(cube);
      gesture.long_press();
    },
  });

  return (
    <View
      ref={ref}
      {...handlers}
      accessibilityRole="button"
      accessibilityLabel={cube_label(cube)}
      style={[STYLE.slot, STYLE.swatch_slot]}
    >
      <Motion to={{ scale: selected ? 1.1 : 1 }} transition={SNAP}>
        <CubeSwatch
          cube={cube}
          size={ITEM}
          radius={ITEM / 2}
          style={selected ? STYLE.swatch_selected : STYLE.swatch_idle}
        />
      </Motion>
    </View>
  );
}

function DecorIconButton({ id }: { id: AssetId }) {
  const active = useInHand(id);

  return (
    <Pressable
      onPress={() => select_asset(id)}
      accessibilityRole="button"
      accessibilityLabel={decor_label(id)}
      style={STYLE.slot}
    >
      <Motion to={{ scale: active ? 1.1 : 1 }} transition={SNAP}>
        <View
          style={[
            STYLE.decor_disc,
            active ? STYLE.swatch_selected : STYLE.swatch_idle,
          ]}
        >
          <Glyph
            glyph={decor_glyph(id)}
            size={16}
            color={active ? STRIP.glyph_on : STRIP.glyph}
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
  switch (kind_of(id)) {
    case KIND_DECOR:
      return <DecorIconButton id={id} />;
    case KIND_MARKER:
      return <MarkerButton />;
    default:
      return <SwatchButton cube={id} gesture={gesture} />;
  }
}

function MarkerButton() {
  const active = useInHand(MARKER);

  return (
    <Pressable
      onPress={() => select_asset(MARKER)}
      accessibilityRole="button"
      accessibilityLabel={PIN_LABEL}
      style={STYLE.slot}
    >
      <Motion to={{ scale: active ? 1.1 : 1 }} transition={SNAP}>
        <View
          style={[
            STYLE.decor_disc,
            active ? STYLE.swatch_selected : STYLE.swatch_idle,
          ]}
        >
          <Glyph
            glyph="bubble.left"
            size={16}
            color={active ? STRIP.glyph_on : STRIP.glyph}
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
      style={STYLE.slot}
    >
      {/* The highlight stays mounted and only its color changes. On Android,
          a view created with its background already set is drawn without its
          rounded corners. */}
      <View
        collapsable={false}
        style={[STYLE.highlight, active && STYLE.tool_active]}
      >
        <Glyph
          glyph={glyph}
          size={CHROME.glyph}
          color={active ? STRIP.glyph_on : STRIP.glyph}
        />
      </View>
    </Pressable>
  );
}

/** `spanPx` is the bar's width, and the strip never grows past it. The caller
 * stops the hotbar's scrolling while the sizes are open. */
export function Strip({
  spanPx,
  scrolls,
  gesture,
}: {
  spanPx: f64;
  scrolls: boolean;
  gesture: Omit<SizeGesture, "press">;
}) {
  const { t } = useTranslation();
  const active_tool = useTool();
  const recent_picks = useRecentPicks();

  return (
    <View style={[STYLE.strip, { maxWidth: spanPx }]}>
      <View style={STYLE.group}>
        <ToolButton
          label={t("world.dock.pointer")}
          glyph="hand.point.up.left"
          active={active_tool === TOOL_POINTER}
          onPress={() => select_tool(TOOL_POINTER)}
        />
        <ToolButton
          label={t("world.dock.eraser")}
          glyph="eraser"
          active={active_tool === TOOL_ERASER}
          onPress={() => select_tool(TOOL_ERASER)}
        />
      </View>

      <View style={STYLE.divider} />

      <ScrollView
        horizontal
        scrollEnabled={scrolls}
        showsHorizontalScrollIndicator={false}
        style={STYLE.picks}
        contentContainerStyle={STYLE.picks_content}
      >
        {recent_picks.map((id) => (
          <PickButton key={id} id={id} gesture={gesture} />
        ))}
      </ScrollView>

      <View style={STYLE.divider} />

      <View style={STYLE.group}>
        <ToolButton
          label={t("world.dock.drawer_tooltip")}
          glyph="archivebox"
          onPress={() => open_modal("drawer")}
        />
      </View>
    </View>
  );
}

export const STRIP = {
  glyph: alpha(COLOR.white, 0.6),
  glyph_on: COLOR.white,
  mark: COLOR.white,
  highlight: alpha(COLOR.white, 0.2),
  line: alpha(COLOR.white, 0.15),
  swatch_line: alpha(COLOR.white, 0.3),
  ground: COLOR.band,
};

/** A slot is one touch target wide. A wider slot would overlap its neighbor,
 * and of two overlapping views the later one in the tree takes the tap. */
export const SLOT: ViewStyle = {
  width: CHROME.target,
  height: CHROME.target,
  alignItems: "center",
  justifyContent: "center",
  flexShrink: 0,
};

const STYLE = StyleSheet.create({
  strip: {
    height: CHROME.control,
    borderRadius: CHROME.control / 2,
    flexDirection: "row",
    alignItems: "center",
  },
  group: {
    flexDirection: "row",
    alignItems: "center",
    padding: (CHROME.control - CHROME.target) / 2,
    flexShrink: 0,
  },
  divider: { width: 1, height: 28, backgroundColor: STRIP.line, flexShrink: 0 },
  picks: { flexShrink: 1, minWidth: 0 },
  picks_content: {
    alignItems: "center",
    paddingHorizontal: (CHROME.control - CHROME.target) / 2,
  },

  slot: SLOT,
  swatch_slot: PAN_X,
  highlight: {
    width: CHROME.selection,
    height: CHROME.selection,
    borderRadius: CHROME.selection / 2,
    alignItems: "center",
    justifyContent: "center",
  },
  tool_active: { backgroundColor: STRIP.highlight },
  decor_disc: {
    width: ITEM,
    height: ITEM,
    borderRadius: ITEM / 2,
    alignItems: "center",
    justifyContent: "center",
    backgroundColor: STRIP.highlight,
  },
  swatch_idle: { boxShadow: `0 0 0 1px ${STRIP.swatch_line}` },
  swatch_selected: { boxShadow: selection_ring(STRIP.ground, STRIP.mark, 1) },
});
