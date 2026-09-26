// @pxl/web/shared/ui/toast/toast.tsx

import { CheckCircle2, Loader2, MapPinOff } from "lucide-react-native";
import { useEffect, useState } from "react";
import { type GestureResponderHandlers, PixelRatio, StyleSheet, Text, useWindowDimensions, View } from "react-native";

import type { f64 } from "@xpute/core/abi/word.ts";
import { Motion, Spin } from "@platform/motion.tsx";
import { LABEL, STATUS } from "@platform/palette.ts";
import { Pill } from "@platform/pill.tsx";
import { Swap } from "@/shared/ui/motion/presence.tsx";
import { type Ease, LEAVE, SNAP, type Spring, type Transition } from "@/shared/ui/motion/transition.ts";
import { alpha, color, font } from "@/shared/ui/theme.ts";
import type { ToastSpec, ToastTone } from "./store.ts";

const SIDE = 32;

/** How long a card stands as a circle, its glyph alone, before it opens. */
const OPEN_AFTER_MS = 450;

/** The check's arrival: under critical damping on purpose, a small bounce. */
const POP: Spring = { type: "spring", stiffness: 260, damping: 12, mass: 0.8 };
/** A glyph arriving otherwise, and any glyph leaving. */
const GLYPH_FADE: Ease = { ...LEAVE, durationMs: 200 };

function glyph(tone: ToastTone) {
  switch (tone) {
    case "loading":
      return (
        <Spin periodMs={1000}>
          <Loader2 size={16} strokeWidth={2.5} color={LABEL.primary} />
        </Spin>
      );
    case "error":
      return <MapPinOff size={16} color={STATUS.failed} />;
    case "ready":
      return <CheckCircle2 size={16} strokeWidth={2.5} color={STATUS.ready} />;
  }
}

interface ToastProps extends ToastSpec {
  /** How present the card is in the deck, and how a change is moved
   * through; it mounts gone and arrives. */
  fade: f64;
  fadeTransition: Transition;
  /** What a finger on the card does. */
  touch?: GestureResponderHandlers;
}

/**
 * One card of the deck: a circle holding a glyph for its tone, which opens
 * into a pill saying what it reports, with a dot at its end, centered on the
 * deck's point so it grows from its middle — on a whole pixel, where a
 * page's pill cuts its shadow cleanly (platform/web/pill.tsx).
 */
export function Toast({ message, tone, fade, fadeTransition, touch }: ToastProps) {
  const error = tone === "error";
  const span = useWindowDimensions().width;
  const [open, setOpen] = useState(false);
  const [content, setContent] = useState<f64>(SIDE);

  useEffect(() => {
    const timer = setTimeout(() => setOpen(true), OPEN_AFTER_MS);
    return () => clearTimeout(timer);
  }, []);

  const width = open ? Math.max(SIDE, content) : SIDE;

  return (
    <Pill
      x={PixelRatio.roundToNearestPixel(-width / 2)}
      width={width}
      height={SIDE}
      span={span}
      fill={error ? color.alarmGround : color.cream}
      // On glass the tint is all the color it carries: the system draws the
      // rim, and a border is the fill's alone (platform glass.tsx).
      tint={error ? alpha(color.alarm, 0.15) : undefined}
      border={{ width: 2, color: error ? color.alarmEdge : alpha(color.white, 0.9) }}
      shadow={error ? "0 6px 20px rgba(220, 38, 38, 0.15)" : "0 6px 20px rgba(0, 0, 0, 0.25)"}
      fade={fade}
      fadeFrom={0}
      fadeTransition={fadeTransition}
      touch={touch}
      onContentWidth={setContent}
    >
      <View style={styles.slot}>
        <Swap
          id={tone}
          from={tone === "ready" ? { opacity: 0, scale: 0.3, rotate: -20 } : { opacity: 0, scale: 0.8 }}
          exit={{ opacity: 0, scale: 0.5 }}
          transition={tone === "ready" ? POP : GLYPH_FADE}
          exitTransition={GLYPH_FADE}
          style={styles.glyph}
        >
          {glyph(tone)}
        </Swap>
      </View>
      <Motion to={open ? {} : { opacity: 0, translateX: -10 }} transition={SNAP}>
        <Text numberOfLines={1} style={[styles.message, { maxWidth: span - 2 * SIDE - 16 }]}>{message}</Text>
      </Motion>
      <View style={styles.slot}>
        <Motion to={open ? {} : { opacity: 0, scale: 0 }} transition={SNAP}>
          <View style={[styles.dot, { backgroundColor: error ? STATUS.failed : tone === "ready" ? STATUS.ready : LABEL.secondary }]} />
        </Motion>
      </View>
    </Pill>
  );
}

const styles = StyleSheet.create({
  slot: { width: SIDE, height: SIDE, alignItems: "center", justifyContent: "center" },
  glyph: { position: "absolute" },
  message: { fontFamily: font.family, fontSize: 13, fontWeight: "600", letterSpacing: -0.3, color: LABEL.primary, paddingHorizontal: 4 },
  dot: { width: 6, height: 6, borderRadius: 3 },
});
