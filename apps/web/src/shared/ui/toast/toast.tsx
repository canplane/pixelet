// @pxl/web/shared/ui/toast/toast.tsx

import { useEffect, useState } from "react";
import { type GestureResponderHandlers, PixelRatio, StyleSheet, Text, useWindowDimensions, View } from "react-native";

import type { f64 } from "@xpute/core/abi/word.ts";
import { Spinner } from "@platform/icon.tsx";
import { Motion } from "@platform/motion.tsx";
import { LABEL, STATUS } from "@platform/palette.ts";
import { Pill } from "@platform/pill.tsx";
import { BackingPlace } from "@/shared/ui/backing-place.ts";
import { Glyph } from "@/shared/ui/glyph.tsx";
import { Swap } from "@/shared/ui/motion/presence.tsx";
import { FADE, SNAP, spring, type Transition } from "@/shared/ui/motion/transition.ts";
import { alpha, chrome, color, shadow, surface, SYSTEM, textStyle, weight } from "@/shared/ui/theme.ts";
import { type ToastSpec, ToastTone } from "./store.ts";

const SIDE = 32;
/** The message's room either side, inside the card. */
const MESSAGE_PAD = 4;
/** Kept clear of the window's sides, outside the card. */
const EDGE = 4;

/** How long a card stands as a circle, its glyph alone, before it opens. */
const OPEN_AFTER_MS = 450;

/** The check's arrival: under critical damping on purpose, a small bounce. */
const POP: Transition = spring({ stiffness: 260, damping: 12, mass: 0.8 });

/** A tone's glyph and its dot: a status as Apple draws one
 * (checkmark.circle.fill, xmark.circle.fill), a white glyph in a disc of the
 * status's color. A stroked glyph in the color alone is too thin to read —
 * green's stroke on the glass stands under 2:1 against it — where a filled
 * disc is read by its shape whatever it stands on. */
const TONE: Record<ToastTone, { mark: "checkmark" | "xmark" | null; status: string }> = {
  // On its way: the green it will turn, faint, as iOS's own green at 30%.
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
  const error = tone === ToastTone.Error;
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
      fill={error ? surface.alarm.fill : color.ground}
      // On glass the tint is all the color it carries: the system draws the
      // rim, and a border is the fill's alone (platform backing.tsx).
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
