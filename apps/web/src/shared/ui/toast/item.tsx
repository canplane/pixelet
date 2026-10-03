// @pxl/web/shared/ui/toast/item.tsx

import { useEffect, useState } from "react";
import { type GestureResponderHandlers, PixelRatio, StyleSheet, Text, View } from "react-native";

import type { f64 } from "@xpute/core/abi/word.ts";
import { Spinner } from "@platform/icon.tsx";
import { Motion } from "@platform/motion.tsx";
import { LABEL, STATUS } from "@platform/palette.ts";
import { Pill } from "@platform/pill.tsx";
import { useWindowSize } from "@platform/viewport.ts";
import { BackingPlace } from "@/shared/ui/backing.ts";
import { Glyph } from "@/shared/ui/glyph.tsx";
import { Swap } from "@/shared/ui/motion/presence.tsx";
import { FADE, SNAP, spring, type Transition } from "@/shared/ui/motion/transition.ts";
import { alpha, chrome, color, shadow, surface, SYSTEM, textStyle, weight } from "@/shared/ui/theme.ts";
import { type ToastSpec, ToastTone } from "./store.ts";

const SIDE = 32;
const MESSAGE_PAD = 4;
const EDGE = 4;

const OPEN_AFTER_MS = 450;

/** Underdamped on purpose: a small bounce. */
const POP: Transition = spring({ stiffness: 260, damping: 12, mass: 0.8 });

/** A filled disc, as Apple's status glyphs: a green stroke on glass stood under 2:1. */
const TONE: Record<ToastTone, { mark: "checkmark" | "xmark" | null; status: string }> = {
  [ToastTone.Loading]: { mark: null, status: alpha(SYSTEM.green, 0.3) },
  [ToastTone.Error]: { mark: "xmark", status: STATUS.failed },
  [ToastTone.Ready]: { mark: "checkmark", status: STATUS.ready },
};

function ToneGlyph({ tone }: { tone: ToastTone }) {
  const { mark, status } = TONE[tone];
  if (!mark) return <Spinner size={chrome.glyph} color={LABEL.primary} />;
  return (
    <View style={[styles.disc, { backgroundColor: status }]}>
      <Glyph glyph={mark} size={12} weight="black" color={color.white} />
    </View>
  );
}

interface ToastProps extends ToastSpec {
  fade: f64;
  fadeTransition: Transition;
  touch?: GestureResponderHandlers;
}

export function Toast({ message, tone, fade, fadeTransition, touch }: ToastProps) {
  const error = tone === ToastTone.Error;
  const span = useWindowSize().width;
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
      fill={error ? surface.alarm.fill : color.ground}
      tint={error ? surface.alarm.glass : undefined}
      place={BackingPlace.OverMap}
      border={{ width: 2, color: error ? surface.alarm.edge : alpha(color.white, 0.9) }}
      shadow={error ? `0 6px 20px ${surface.alarm.shadow}` : shadow.lift}
      fade={fade}
      fadeFrom={0}
      fadeTransition={fadeTransition}
      touch={touch}
      onContentWidth={setContent}
    >
      <View style={styles.slot}>
        <Swap
          id={tone}
          from={tone === ToastTone.Ready ? { opacity: 0, scale: 0.3, rotate: -20 } : { opacity: 0, scale: 0.8 }}
          exit={{ opacity: 0, scale: 0.5 }}
          transition={tone === ToastTone.Ready ? POP : FADE}
          exitTransition={FADE}
          style={styles.glyph}
        >
          <ToneGlyph tone={tone} />
        </Swap>
      </View>
      <Motion to={open ? {} : { opacity: 0, translateX: -10 }} transition={SNAP}>
        <Text numberOfLines={1} style={[styles.message, { maxWidth: span - 2 * SIDE - 2 * MESSAGE_PAD - 2 * EDGE }]}>{message}</Text>
      </Motion>
      <View style={styles.slot}>
        <Motion to={open ? {} : { opacity: 0, scale: 0 }} transition={SNAP}>
          <View style={[styles.dot, { backgroundColor: TONE[tone].status }]} />
        </Motion>
      </View>
    </Pill>
  );
}

const styles = StyleSheet.create({
  slot: { width: SIDE, height: SIDE, alignItems: "center", justifyContent: "center" },
  glyph: { position: "absolute" },
  disc: { width: chrome.glyph, height: chrome.glyph, borderRadius: chrome.glyph / 2, alignItems: "center", justifyContent: "center" },
  message: { ...textStyle.footnote, fontWeight: weight.semibold, letterSpacing: -0.3, color: LABEL.primary, paddingHorizontal: MESSAGE_PAD },
  dot: { width: 8, height: 8, borderRadius: 4 },
});
