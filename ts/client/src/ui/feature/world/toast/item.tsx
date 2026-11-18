// @pxl/client/ui/feature/world/toast/item.tsx

import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { PixelRatio, StyleSheet, Text, View } from "react-native";

import type { f64 } from "@xpute/kit/abi/word.ts";
import { Spinner } from "@/ui/shared/component/icon.tsx";
import { Motion } from "@/ui/shared/component/motion/motion.tsx";
import { STATUS } from "@/ui/shared/component/palette.ts";
import { Pill } from "@/ui/feature/world/pill.tsx";
import { BackingPlace } from "@/ui/shared/component/backing.tsx";
import { Glyph } from "@/ui/shared/component/glyph.tsx";
import { Swap } from "@/ui/shared/component/motion/presence.tsx";
import {
  FADE,
  SNAP,
  POP,
  type Transition,
} from "@/ui/shared/component/motion/transition.ts";
import {
  alpha,
  CHROME,
  COLOR,
  SHADOW,
  SYSTEM,
  TEXT_SCALE_CAP,
  TEXT_STYLE,
} from "@/ui/shared/component/theme.ts";
import { type ToastSpec, ToastTone } from "./store.ts";
import { useWindowSize } from "@/service/platform/viewport.ts";

export const TOAST_SIDE = 32;
const MESSAGE_PAD = 4;
const EDGE = 4;

const OPEN_AFTER_MS = 450;

/** A tone's mark is white, on a faint white disc over the band, and its color
 * is the dot's (`status`). */
const TONE: Record<
  ToastTone,
  { mark: "checkmark" | "xmark" | null; status: string }
> = {
  [ToastTone.LOADING]: { mark: null, status: alpha(SYSTEM.green, 0.3) },
  [ToastTone.ERROR]: { mark: "xmark", status: STATUS.failed },
  [ToastTone.READY]: { mark: "checkmark", status: STATUS.ready },
};

function ToneGlyph({ tone }: { tone: ToastTone }) {
  const { mark } = TONE[tone];
  if (!mark) return <Spinner size={CHROME.glyph} color={COLOR.white} />;
  return (
    <View style={[STYLE.disc, { backgroundColor: alpha(COLOR.white, 0.2) }]}>
      <Glyph glyph={mark} size={12} weight="black" color={COLOR.white} />
    </View>
  );
}

interface ToastProps {
  spec: ToastSpec;
  fade: f64;
  fadeTransition: Transition;
  touch?: boolean;
  touchBelow?: f64;
}

export function Toast({
  spec: { key, tone },
  fade,
  fadeTransition,
  touch,
  touchBelow,
}: ToastProps) {
  const { t } = useTranslation();
  const windowWidth = useWindowSize().width;
  const [open, setOpen] = useState(false);
  const [contentWidth, setContentWidth] = useState<f64>(TOAST_SIDE);

  useEffect(() => {
    const timer = setTimeout(() => setOpen(true), OPEN_AFTER_MS);
    return () => clearTimeout(timer);
  }, []);

  const width = open ? Math.max(TOAST_SIDE, contentWidth) : TOAST_SIDE;

  return (
    <Pill
      x={PixelRatio.roundToNearestPixel(-width / 2)}
      width={width}
      height={TOAST_SIDE}
      spanPx={windowWidth}
      fill={COLOR.band}
      solid
      place={BackingPlace.OVER_MAP}
      shadow={tone === ToastTone.ERROR ? SHADOW.alarm : SHADOW.lift}
      fade={fade}
      fadeFrom={0}
      fadeTransition={fadeTransition}
      touch={touch}
      touchBelow={touchBelow}
      onContentWidth={setContentWidth}
    >
      <View style={STYLE.slot}>
        <Swap
          id={tone}
          from={
            tone === ToastTone.READY
              ? { opacity: 0, scale: 0.3, rotate: -20 }
              : { opacity: 0, scale: 0.8 }
          }
          exit={{ opacity: 0, scale: 0.5 }}
          transition={tone === ToastTone.READY ? POP : FADE}
          exitTransition={FADE}
          style={STYLE.glyph}
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
          maxFontSizeMultiplier={TEXT_SCALE_CAP.chrome}
          style={[
            STYLE.message,
            {
              maxWidth:
                windowWidth - 2 * TOAST_SIDE - 2 * MESSAGE_PAD - 2 * EDGE,
            },
          ]}
        >
          {t(key)}
        </Text>
      </Motion>
      <View style={STYLE.slot}>
        <Motion to={open ? {} : { opacity: 0, scale: 0 }} transition={SNAP}>
          <View style={[STYLE.dot, { backgroundColor: TONE[tone].status }]} />
        </Motion>
      </View>
    </Pill>
  );
}

const STYLE = StyleSheet.create({
  slot: {
    width: TOAST_SIDE,
    height: TOAST_SIDE,
    alignItems: "center",
    justifyContent: "center",
  },
  glyph: { position: "absolute" },
  disc: {
    width: CHROME.glyph,
    height: CHROME.glyph,
    borderRadius: CHROME.glyph / 2,
    alignItems: "center",
    justifyContent: "center",
  },
  message: {
    ...TEXT_STYLE.notice,
    color: COLOR.white,
    paddingHorizontal: MESSAGE_PAD,
  },
  dot: { width: 8, height: 8, borderRadius: 4 },
});
