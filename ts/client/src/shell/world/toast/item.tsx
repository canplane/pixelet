// @pxl/client/shell/world/toast/item.tsx

import { useEffect, useState } from "react";
import {
  type GestureResponderHandlers,
  PixelRatio,
  StyleSheet,
  Text,
  View,
} from "react-native";

import type { f64 } from "@xpute/kit/abi/word.ts";
import { Spinner } from "@/shell/ui/icon.tsx";
import { Motion } from "@/shell/ui/motion/motion.tsx";
import { LABEL, STATUS } from "@/shell/ui/palette.ts";
import { Pill } from "@/shell/world/pill.tsx";
import { BackingPlace } from "@/shell/ui/backing.tsx";
import { Glyph } from "@/shell/ui/glyph.tsx";
import { Swap } from "@/shell/ui/motion/presence.tsx";
import {
  FADE,
  SNAP,
  spring,
  type Transition,
} from "@/shell/ui/motion/transition.ts";
import {
  alpha,
  chrome,
  color,
  shadow,
  surface,
  SYSTEM,
  textScale,
  textStyle,
} from "@/shell/ui/theme.ts";
import { type ToastSpec, ToastTone } from "./store.ts";
import { bracket } from "@/bracket.tsx";

export const TOAST_SIDE = 32;
const MESSAGE_PAD = 4;
const EDGE = 4;

const OPEN_AFTER_MS = 450;

/** Underdamped on purpose: a small bounce. */
const POP: Transition = spring({ stiffness: 260, damping: 12, mass: 0.8 });

/** A filled disc, as Apple's status glyphs: a green stroke on glass stood under 2:1. */
const TONE: Record<
  ToastTone,
  { mark: "checkmark" | "xmark" | null; status: string }
> = {
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
  touchBelow?: f64;
}

export function Toast({
  message,
  tone,
  fade,
  fadeTransition,
  touch,
  touchBelow,
}: ToastProps) {
  const error = tone === ToastTone.Error;
  const span = bracket.view.useWindowSize().width;
  const [open, setOpen] = useState(false);
  const [content, setContent] = useState<f64>(TOAST_SIDE);

  useEffect(() => {
    const timer = setTimeout(() => setOpen(true), OPEN_AFTER_MS);
    return () => clearTimeout(timer);
  }, []);

  const width = open ? Math.max(TOAST_SIDE, content) : TOAST_SIDE;

  return (
    <Pill
      x={PixelRatio.roundToNearestPixel(-width / 2)}
      width={width}
      height={TOAST_SIDE}
      span={span}
      fill={error ? surface.alarm.fill : color.ground}
      tint={error ? surface.alarm.glass : undefined}
      place={BackingPlace.OverMap}
      border={{
        width: 2,
        color: error ? surface.alarm.edge : alpha(color.white, 0.9),
      }}
      shadow={error ? `0 6px 20px ${surface.alarm.shadow}` : shadow.lift}
      fade={fade}
      fadeFrom={0}
      fadeTransition={fadeTransition}
      touch={touch}
      touchBelow={touchBelow}
      onContentWidth={setContent}
    >
      <View style={styles.slot}>
        <Swap
          id={tone}
          from={
            tone === ToastTone.Ready
              ? { opacity: 0, scale: 0.3, rotate: -20 }
              : { opacity: 0, scale: 0.8 }
          }
          exit={{ opacity: 0, scale: 0.5 }}
          transition={tone === ToastTone.Ready ? POP : FADE}
          exitTransition={FADE}
          style={styles.glyph}
        >
          <ToneGlyph tone={tone} />
        </Swap>
      </View>
      <Motion
        to={open ? {} : { opacity: 0, translateX: -10 }}
        transition={SNAP}
      >
        <Text
          numberOfLines={1}
          maxFontSizeMultiplier={textScale.chrome}
          style={[
            styles.message,
            {
              maxWidth: span - 2 * TOAST_SIDE - 2 * MESSAGE_PAD - 2 * EDGE,
            },
          ]}
        >
          {message}
        </Text>
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
  slot: {
    width: TOAST_SIDE,
    height: TOAST_SIDE,
    alignItems: "center",
    justifyContent: "center",
  },
  glyph: { position: "absolute" },
  disc: {
    width: chrome.glyph,
    height: chrome.glyph,
    borderRadius: chrome.glyph / 2,
    alignItems: "center",
    justifyContent: "center",
  },
  message: {
    ...textStyle.notice,
    color: LABEL.primary,
    paddingHorizontal: MESSAGE_PAD,
  },
  dot: { width: 8, height: 8, borderRadius: 4 },
});
