// @pxl/client/ui/world/dock/popover.tsx

/**
 * The sizes of a cube, shown above the dock's card. A press on a cube's swatch
 * shows a small dot above it, the seed. Held, the seed opens into the sizes,
 * one mark for each scale, and the finger picks one by lifting over it.
 */

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

/** How far above the card the seed and the sizes stand. */
const POPOVER_LIFT_PX = 6;
const POPOVER_GAP_PX = 8;
const POPOVER_PAD_Y = 8;
const POPOVER_WIDTH = chrome.target + 12;
const POPOVER_PITCH = chrome.target + POPOVER_GAP_PX;

/** The diameter of the coarsest scale's mark. Each step finer halves the
 * mark's area rather than its width, so that the finest mark stays visible:
 * 28, 20, 14 and 10 pixels. */
const MARK_COARSEST = 28;
function sizeMark(scaleSteps: i32): f64 {
  return round(MARK_COARSEST * Math.SQRT2 ** scaleSteps);
}

/** The scales from top to bottom. The finest comes first, so that the
 * coarsest stands nearest the swatch. */
const SCALES: readonly i32[] = Array.from(
  { length: OBJECT_SCALES },
  (_, i) => i + 1 - OBJECT_SCALES,
);

/** The seed's diameter in pixels. */
const SEED = 10;

/** How long a key's flash keeps the seed or the sizes up: long enough to read
 * which size is held. */
const FLASH_MS = 900;

/** `centerX` is the swatch's center, measured from the bar's left edge. `bar`
 * is the bar's top-left corner in the window, and the bar's top is the
 * card's. */
export interface Popover {
  centerX: f64;
  bar: [f64, f64];
  highlighted: i32 | null;
  /** Whether the sizes are open. Otherwise only the seed shows. */
  open: boolean;
}

/** The scale whose mark lies under `x` and `y`, which are in the window's
 * coordinates, as the responder gives them. */
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

/** `bar` is the dock's bar. The popover's place is measured from it. */
export function usePopover(bar: React.RefObject<View | null>): {
  popover: Popover | null;
  gesture: Omit<SizeGesture, "press">;
} {
  const [popover, setPopover] = useState<Popover | null>(null);
  // Measuring the swatch takes a moment, and the finger can lift before the
  // measurement arrives. A measurement that arrives after the lift is dropped.
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

/** The seed or the sizes over the swatch. The bar draws them rather than the
 * swatch, because the swatch's strip scrolls and would clip them. */
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

function SizeMarks({ highlighted }: { highlighted: i32 | null }) {
  const { t } = useTranslation();
  const scale = useScale();

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
                    : scale === scaleSteps
                      ? styles.markCurrent
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
  // The open sizes take every pointer event, so that the world under them
  // shows no hover.
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
  markCurrent: { backgroundColor: alpha(STRIP.mark, 0.8) },
  markHighlighted: {
    backgroundColor: STRIP.mark,
    boxShadow: `0 0 0 1px ${STRIP.mark}`,
  },
});
