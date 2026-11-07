// @pxl/client/ui/world/dock/popover.tsx

/** A swatch's sizes above the card: a press shows their seed, and held, it grows into them. */

import { useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { StyleSheet, View } from "react-native";

import type { f64, i32 } from "@xpute/kit/abi/word.ts";
import { round } from "@xpute/kit/math/scalar.ts";
import { Backing, BackingPlace } from "@/ui/component/backing.tsx";
import { Motion } from "@/ui/component/motion/motion.tsx";
import { SNAP } from "@/ui/component/motion/transition.ts";
import { touches } from "@/ui/component/touch.ts";
import { alpha, chrome, material, shadow } from "@/ui/component/theme.ts";
import { selectScale, useScale } from "./store.ts";
import { OBJECT_SCALES } from "@/ui/world/object.spec.ts";
import { DOCK_HEIGHT } from "./card.tsx";
import { type SizeGesture, STRIP } from "./strip.tsx";

/** Above the card, where the sizes and their seed stand. */
const POPOVER_LIFT_PX = 6;
const POPOVER_GAP_PX = 8;
const POPOVER_PAD_Y = 8;
const POPOVER_WIDTH = chrome.target + 12;
const POPOVER_PITCH = chrome.target + POPOVER_GAP_PX;

/** A dot whose area halves per scale step, so the finest stays visible: 28, 20, 14, 10. */
const MARK_COARSEST = 28;
function sizeMark(scaleSteps: i32): f64 {
  return round(MARK_COARSEST * Math.SQRT2 ** scaleSteps);
}

/** Finest first, so the coarsest stands nearest the swatch. */
const SCALES: readonly i32[] = Array.from(
  { length: OBJECT_SCALES },
  (_, i) => i + 1 - OBJECT_SCALES,
);

/** The seed of the sizes, at their foot. */
const SEED = 10;

/** How long a key's flash leaves the seed or the sizes up, long enough to read the one held. */
const FLASH_MS = 900;

/** In the bar's frame, whose top is the card's; `bar` is where the bar is on the page. */
export interface Popover {
  centerX: f64;
  bar: [f64, f64];
  highlighted: i32 | null;
  /** The sizes open; else their seed. */
  open: boolean;
}

/** `x` and `y` on the page. */
function scaleAt(at: Popover, x: f64, y: f64): i32 | null {
  if (Math.abs(x - at.bar[0] - at.centerX) > POPOVER_WIDTH / 2) return null;
  const top =
    at.bar[1] -
    POPOVER_LIFT_PX -
    (2 * POPOVER_PAD_Y + OBJECT_SCALES * POPOVER_PITCH - POPOVER_GAP_PX);
  const along = y - top - POPOVER_PAD_Y;
  if (along < 0) return null;
  const row = Math.floor(along / POPOVER_PITCH);
  if (row >= OBJECT_SCALES || along - row * POPOVER_PITCH > chrome.target) {
    return null;
  }
  return SCALES[row];
}

/** Where the popover stands, and what a swatch's gesture does to it; `bar` is the dock's, whose place it is measured from. */
export function usePopover(bar: React.RefObject<View | null>): {
  popover: Popover | null;
  gesture: Omit<SizeGesture, "press">;
} {
  const [popover, setPopover] = useState<Popover | null>(null);
  // A release can come before the swatch's place is measured.
  const pressing = useRef(false);
  const flashTimer = useRef<ReturnType<typeof setTimeout> | undefined>(
    undefined,
  );

  const gesture: Omit<SizeGesture, "press"> = {
    pressIn: (swatch) => {
      pressing.current = true;
      swatch.measureInWindow((sx, _sy, sw) => {
        bar.current?.measureInWindow((bx, by) => {
          if (!pressing.current) return;
          setPopover({
            centerX: sx + sw / 2 - bx,
            bar: [bx, by],
            highlighted: null,
            open: false,
          });
        });
      });
    },
    flash: (swatch, open) => {
      clearTimeout(flashTimer.current);
      swatch.measureInWindow((sx, _sy, sw) => {
        bar.current?.measureInWindow((bx, by) =>
          setPopover({
            centerX: sx + sw / 2 - bx,
            bar: [bx, by],
            highlighted: null,
            open,
          }),
        );
      });
      flashTimer.current = setTimeout(() => setPopover(null), FLASH_MS);
    },
    longPress: () => setPopover((f) => f && { ...f, open: true }),
    move: (x, y) => {
      if (!popover?.open) return;
      const highlighted = scaleAt(popover, x, y);
      if (highlighted !== popover.highlighted) {
        setPopover({ ...popover, highlighted });
      }
    },
    release: (x, y) => {
      const scaleSteps = popover?.open ? scaleAt(popover, x, y) : null;
      if (scaleSteps !== null) selectScale(scaleSteps);
    },
    end: () => {
      pressing.current = false;
      setPopover(null);
    },
  };

  return { popover, gesture };
}

/** The seed or the sizes over the swatch. Drawn from the bar, not the swatch, whose scrolling strip would clip it. */
export function SizePopover({ popover }: { popover: Popover }) {
  return (
    <View
      style={[
        styles.popoverAt,
        popover.open ? styles.popoverTakes : touches.block,
        { left: popover.centerX - POPOVER_WIDTH / 2 },
      ]}
    >
      {popover.open ? (
        <SizeMarks highlighted={popover.highlighted} />
      ) : (
        <Backing
          fill={STRIP.ground}
          material={material.popover}
          place={BackingPlace.OverMap}
          style={styles.seed}
        />
      )}
    </View>
  );
}

/** The sizes open, a mark a scale. */
function SizeMarks({ highlighted }: { highlighted: i32 | null }) {
  const { t } = useTranslation();
  const held = useScale();

  return (
    <Backing
      fill={STRIP.ground}
      material={material.popover}
      place={BackingPlace.OverMap}
      style={styles.popover}
    >
      {SCALES.map((scaleSteps) => {
        const mark = sizeMark(scaleSteps);
        return (
          <View
            key={scaleSteps}
            accessibilityLabel={t("world.dock.object_size", {
              n: OBJECT_SCALES + scaleSteps,
            })}
            style={styles.popoverItem}
          >
            <Motion
              to={{ scale: highlighted === scaleSteps ? 1.25 : 1 }}
              transition={SNAP}
            >
              <View
                style={[
                  { width: mark, height: mark, borderRadius: mark / 2 },
                  highlighted === scaleSteps
                    ? styles.markHighlighted
                    : held === scaleSteps
                      ? styles.markHeld
                      : styles.markIdle,
                ]}
              />
            </Motion>
          </View>
        );
      })}
    </Backing>
  );
}

const styles = StyleSheet.create({
  popoverAt: {
    position: "absolute",
    bottom: DOCK_HEIGHT + POPOVER_LIFT_PX,
    width: POPOVER_WIDTH,
    zIndex: 10,
  },
  // Takes the pointer whole, so the world under it draws no hover.
  popoverTakes: { pointerEvents: "box-only" },
  seed: {
    alignSelf: "center",
    width: SEED,
    height: SEED,
    borderRadius: SEED / 2,
    boxShadow: shadow.lift,
  },
  popover: {
    alignItems: "center",
    gap: POPOVER_GAP_PX,
    paddingVertical: POPOVER_PAD_Y,
    borderRadius: POPOVER_WIDTH / 2,
    boxShadow: shadow.lift,
  },
  popoverItem: {
    width: chrome.target,
    height: chrome.target,
    alignItems: "center",
    justifyContent: "center",
  },
  markIdle: { backgroundColor: alpha(STRIP.mark, 0.4) },
  markHeld: { backgroundColor: alpha(STRIP.mark, 0.8) },
  markHighlighted: {
    backgroundColor: STRIP.mark,
    boxShadow: `0 0 0 1px ${STRIP.mark}`,
  },
});
