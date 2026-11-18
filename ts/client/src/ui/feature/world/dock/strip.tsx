// @pxl/client/ui/feature/world/dock/strip.tsx

import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import {
  Pressable,
  ScrollView,
  StyleSheet,
  View,
  type ViewInstance,
  type ViewStyle,
} from "react-native";

import {
  GestureDetector,
  usePanGesture,
  useSimultaneousGestures,
  useTapGesture,
} from "react-native-gesture-handler";

import type { f64 } from "@xpute/kit/abi/word.ts";
import type { GlyphName } from "@/ui/shared/component/icon.tsx";
import { Glyph } from "@/ui/shared/component/glyph.tsx";
import { Motion } from "@/ui/shared/component/motion/motion.tsx";
import { SNAP } from "@/ui/shared/component/motion/transition.ts";
import { alpha, CHROME, COLOR } from "@/ui/shared/component/theme.ts";
import { openModal } from "@/ui/shared/modal.ts";
import { SLOP_PX } from "@/ui/feature/world/deck.ts";
import {
  selectTool,
  selectAsset,
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
  cubeLabel,
  decorLabel,
  kindOf,
  MARKER,
} from "@/ui/shared/asset/catalog.ts";
import { PIN_LABEL } from "@/ui/shared/asset/marker.spec.ts";
import {
  CubeSwatch,
  decorGlyph,
  selectionRing,
} from "@/ui/shared/asset/swatch.tsx";

const ITEM = 24;

/** How long a swatch is held before its sizes open. A finger that moves
 * before then drags instead: sideways it scrolls the strip, and up or down
 * it turns the deck. */
const LONG_PRESS_MS = 300;

/** The handlers of a swatch's gesture. `press`, `pressIn` and `longPress` mean
 * what they mean on React Native's `Pressable`. `release` is a lift while the
 * sizes are open. `end` closes the sizes; it comes on every lift, on a drag,
 * and when the gesture is taken away. */
export interface SizeGesture {
  press: () => void;
  pressIn: (swatch: ViewInstance) => void;
  /** A key picked the cube or stepped its size. Shows the seed, or the sizes
   * when `open`, over the swatch for a moment, as a press would. */
  flash: (swatch: ViewInstance, open: boolean) => void;
  longPress: () => void;
  move: (x: f64, y: f64) => void;
  release: (x: f64, y: f64) => void;
  end: () => void;
}

/** A swatch's gesture: a lift before `LONG_PRESS_MS` presses, and a hold that
 * long opens the sizes, which the finger then moves over and lifts on. */
function useSizeGesture(
  swatch: React.RefObject<ViewInstance | null>,
  g: SizeGesture,
) {
  const hold = usePanGesture({
    runOnJS: true,
    activateAfterLongPress: LONG_PRESS_MS,
    onBegin: () => {
      if (swatch.current) g.pressIn(swatch.current);
    },
    onActivate: g.longPress,
    onUpdate: (e) => g.move(e.absoluteX, e.absoluteY),
    onDeactivate: (e) => {
      if (!e.canceled) g.release(e.absoluteX, e.absoluteY);
    },
    onFinalize: g.end,
  });
  const press = useTapGesture({
    runOnJS: true,
    maxDuration: LONG_PRESS_MS,
    maxDistance: SLOP_PX,
    onActivate: g.press,
  });
  return useSimultaneousGestures(hold, press);
}

function SwatchButton({
  cube,
  gesture,
}: {
  cube: Cube;
  gesture: Omit<SizeGesture, "press">;
}) {
  const selected = useInHand(cube);
  const ref = useRef<ViewInstance | null>(null);
  const swatchFlash = useSwatchFlash();
  useEffect(() => {
    if (swatchFlash.cube === cube && ref.current) {
      gesture.flash(ref.current, swatchFlash.open);
    }
  }, [swatchFlash]);

  const sizeGesture = useSizeGesture(ref, {
    ...gesture,
    press: () => selectAsset(cube),
    longPress: () => {
      selectAsset(cube);
      gesture.longPress();
    },
  });

  return (
    <GestureDetector gesture={sizeGesture} touchAction="pan-x">
      <View
        ref={ref}
        accessibilityRole="button"
        accessibilityLabel={cubeLabel(cube)}
        style={STYLE.slot}
      >
        <Motion to={{ scale: selected ? 1.1 : 1 }} transition={SNAP}>
          <CubeSwatch
            cube={cube}
            size={ITEM}
            radius={ITEM / 2}
            style={selected ? STYLE.swatchSelected : STYLE.swatchIdle}
          />
        </Motion>
      </View>
    </GestureDetector>
  );
}

function DecorIconButton({ id }: { id: AssetId }) {
  const active = useInHand(id);

  return (
    <Pressable
      onPress={() => selectAsset(id)}
      accessibilityRole="button"
      accessibilityLabel={decorLabel(id)}
      style={STYLE.slot}
    >
      <Motion to={{ scale: active ? 1.1 : 1 }} transition={SNAP}>
        <View
          style={[
            STYLE.decorDisc,
            active ? STYLE.swatchSelected : STYLE.swatchIdle,
          ]}
        >
          <Glyph
            glyph={decorGlyph(id)}
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
      onPress={() => selectAsset(MARKER)}
      accessibilityRole="button"
      accessibilityLabel={PIN_LABEL}
      style={STYLE.slot}
    >
      <Motion to={{ scale: active ? 1.1 : 1 }} transition={SNAP}>
        <View
          style={[
            STYLE.decorDisc,
            active ? STYLE.swatchSelected : STYLE.swatchIdle,
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
      style={STYLE.slot}
    >
      {/* The highlight stays mounted and only its color changes. On Android,
          a view created with its background already set is drawn without its
          rounded corners. */}
      <View
        collapsable={false}
        style={[STYLE.highlight, active && STYLE.toolActive]}
      >
        <Glyph
          glyph={glyph}
          size={CHROME.glyph}
          color={active ? STRIP.glyphOn : STRIP.glyph}
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
  const activeTool = useTool();
  const recentPicks = useRecentPicks();

  return (
    <View style={[STYLE.strip, { maxWidth: spanPx }]}>
      <View style={STYLE.group}>
        <ToolButton
          label={t("world.dock.pointer")}
          glyph="cursorarrow"
          active={activeTool === TOOL_POINTER}
          onPress={() => selectTool(TOOL_POINTER)}
        />
        <ToolButton
          label={t("world.dock.eraser")}
          glyph="eraser"
          active={activeTool === TOOL_ERASER}
          onPress={() => selectTool(TOOL_ERASER)}
        />
      </View>

      <View style={STYLE.divider} />

      <ScrollView
        horizontal
        scrollEnabled={scrolls}
        showsHorizontalScrollIndicator={false}
        style={STYLE.picks}
        contentContainerStyle={STYLE.picksContent}
      >
        {recentPicks.map((id) => (
          <PickButton key={id} id={id} gesture={gesture} />
        ))}
      </ScrollView>

      <View style={STYLE.divider} />

      <View style={STYLE.group}>
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
  glyph: alpha(COLOR.white, 0.6),
  glyphOn: COLOR.white,
  mark: COLOR.white,
  highlight: alpha(COLOR.white, 0.2),
  line: alpha(COLOR.white, 0.15),
  swatchLine: alpha(COLOR.white, 0.3),
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
  picksContent: {
    alignItems: "center",
    paddingHorizontal: (CHROME.control - CHROME.target) / 2,
  },

  slot: SLOT,
  highlight: {
    width: CHROME.selection,
    height: CHROME.selection,
    borderRadius: CHROME.selection / 2,
    alignItems: "center",
    justifyContent: "center",
  },
  toolActive: { backgroundColor: STRIP.highlight },
  decorDisc: {
    width: ITEM,
    height: ITEM,
    borderRadius: ITEM / 2,
    alignItems: "center",
    justifyContent: "center",
    backgroundColor: STRIP.highlight,
  },
  swatchIdle: { boxShadow: `0 0 0 1px ${STRIP.swatchLine}` },
  swatchSelected: { boxShadow: selectionRing(STRIP.ground, STRIP.mark, 1) },
});
