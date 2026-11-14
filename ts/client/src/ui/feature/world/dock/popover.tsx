// @pxl/client/ui/feature/world/dock/popover.tsx

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
import { Backing, BackingPlace } from "@/ui/shared/component/backing.tsx";
import { Motion } from "@/ui/shared/component/motion/motion.tsx";
import { SNAP } from "@/ui/shared/component/motion/transition.ts";
import { TOUCH } from "@/ui/shared/component/touch.ts";
import {
  alpha,
  CHROME,
  MATERIAL,
  SHADOW,
} from "@/ui/shared/component/theme.ts";
import { select_scale, useScale } from "./store.ts";
import { OBJECT_SCALE_CNT } from "@/ui/feature/world/object.spec.ts";
import { DOCK_HEIGHT } from "./card.tsx";
import { type SizeGesture, STRIP } from "./strip.tsx";

const POPOVER_LIFT_PX = 6;
const POPOVER_GAP_PX = 8;
const POPOVER_PAD_Y = 8;
const POPOVER_WIDTH = CHROME.target + 12;
const POPOVER_PITCH = CHROME.target + POPOVER_GAP_PX;

/** The diameter of the coarsest scale's mark. Each step finer halves the
 * mark's area rather than its width, so that the finest mark stays visible:
 * 28, 20, 14 and 10 pixels. */
const MARK_COARSEST = 28;
function size_mark(scale_step: i32): f64 {
  return round(MARK_COARSEST * Math.SQRT2 ** scale_step);
}

/** The scales from top to bottom. The finest comes first, so that the
 * coarsest stands nearest the swatch. */
const SCALES: readonly i32[] = Array.from(
  { length: OBJECT_SCALE_CNT },
  (_, i) => i + 1 - OBJECT_SCALE_CNT,
);

/** The seed's diameter in pixels. */
const SEED = 10;

/** How long a key's flash keeps the seed or the sizes up: long enough to read
 * which size is held. */
const FLASH_MS = 900;

/** `center_x` is the swatch's center, measured from the bar's left edge. `bar`
 * is the bar's top-left corner in the window, and the bar's top is the
 * card's. */
export interface Popover {
  center_x: f64;
  bar: [f64, f64];
  highlighted: i32 | null;
  /** Whether the sizes are open. Otherwise only the seed shows. */
  open: boolean;
}

/** The scale whose mark lies under `x` and `y`, which are in the window's
 * coordinates, as the responder gives them. */
function scale_at(at: Popover, x: f64, y: f64): i32 | null {
  if (Math.abs(x - at.bar[0] - at.center_x) > POPOVER_WIDTH / 2) return null;
  const top =
    at.bar[1] -
    POPOVER_LIFT_PX -
    (2 * POPOVER_PAD_Y + OBJECT_SCALE_CNT * POPOVER_PITCH - POPOVER_GAP_PX);
  const along = y - top - POPOVER_PAD_Y;
  if (along < 0) return null;
  const row = Math.floor(along / POPOVER_PITCH);
  if (row >= OBJECT_SCALE_CNT || along - row * POPOVER_PITCH > CHROME.target) {
    return null;
  }
  return SCALES[row];
}

export function usePopover(bar: React.RefObject<View | null>): {
  popover: Popover | null;
  gesture: Omit<SizeGesture, "press">;
} {
  const [popover, set_popover] = useState<Popover | null>(null);
  // Measuring the swatch takes a moment, and the finger can lift before the
  // measurement arrives. A measurement that arrives after the lift is dropped.
  const pressing = useRef(false);
  const flash_timer = useRef<ReturnType<typeof setTimeout> | undefined>(
    undefined,
  );

  const gesture: Omit<SizeGesture, "press"> = {
    press_in: (swatch) => {
      pressing.current = true;
      swatch.measureInWindow((sx, _sy, sw) => {
        bar.current?.measureInWindow((bx, by) => {
          if (!pressing.current) return;
          set_popover({
            center_x: sx + sw / 2 - bx,
            bar: [bx, by],
            highlighted: null,
            open: false,
          });
        });
      });
    },
    flash: (swatch, open) => {
      clearTimeout(flash_timer.current);
      swatch.measureInWindow((sx, _sy, sw) => {
        bar.current?.measureInWindow((bx, by) =>
          set_popover({
            center_x: sx + sw / 2 - bx,
            bar: [bx, by],
            highlighted: null,
            open,
          }),
        );
      });
      flash_timer.current = setTimeout(() => set_popover(null), FLASH_MS);
    },
    long_press: () => set_popover((f) => f && { ...f, open: true }),
    move: (x, y) => {
      if (!popover?.open) return;
      const highlighted = scale_at(popover, x, y);
      if (highlighted !== popover.highlighted) {
        set_popover({ ...popover, highlighted });
      }
    },
    release: (x, y) => {
      const scale_step = popover?.open ? scale_at(popover, x, y) : null;
      if (scale_step !== null) select_scale(scale_step);
    },
    end: () => {
      pressing.current = false;
      set_popover(null);
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
        STYLE.popover_at,
        popover.open ? STYLE.popover_takes : TOUCH.block,
        { left: popover.center_x - POPOVER_WIDTH / 2 },
      ]}
    >
      {popover.open ? (
        <SizeMarks highlighted={popover.highlighted} />
      ) : (
        <Backing
          fill={STRIP.ground}
          material={MATERIAL.popover}
          place={BackingPlace.OVER_MAP}
          style={STYLE.seed}
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
      material={MATERIAL.popover}
      place={BackingPlace.OVER_MAP}
      style={STYLE.popover}
    >
      {SCALES.map((scale_step) => {
        const mark = size_mark(scale_step);
        return (
          <View
            key={scale_step}
            accessibilityLabel={t("world.dock.object_size", {
              n: OBJECT_SCALE_CNT + scale_step,
            })}
            style={STYLE.popover_item}
          >
            <Motion
              to={{ scale: highlighted === scale_step ? 1.25 : 1 }}
              transition={SNAP}
            >
              <View
                style={[
                  { width: mark, height: mark, borderRadius: mark / 2 },
                  highlighted === scale_step
                    ? STYLE.mark_highlighted
                    : scale === scale_step
                      ? STYLE.mark_current
                      : STYLE.mark_idle,
                ]}
              />
            </Motion>
          </View>
        );
      })}
    </Backing>
  );
}

const STYLE = StyleSheet.create({
  popover_at: {
    position: "absolute",
    bottom: DOCK_HEIGHT + POPOVER_LIFT_PX,
    width: POPOVER_WIDTH,
    zIndex: 10,
  },
  // The open sizes take every pointer event, so that the world under them
  // shows no hover.
  popover_takes: { pointerEvents: "box-only" },
  seed: {
    alignSelf: "center",
    width: SEED,
    height: SEED,
    borderRadius: SEED / 2,
    boxShadow: SHADOW.lift,
  },
  popover: {
    alignItems: "center",
    gap: POPOVER_GAP_PX,
    paddingVertical: POPOVER_PAD_Y,
    borderRadius: POPOVER_WIDTH / 2,
    boxShadow: SHADOW.lift,
  },
  popover_item: {
    width: CHROME.target,
    height: CHROME.target,
    alignItems: "center",
    justifyContent: "center",
  },
  mark_idle: { backgroundColor: alpha(STRIP.mark, 0.4) },
  mark_current: { backgroundColor: alpha(STRIP.mark, 0.8) },
  mark_highlighted: {
    backgroundColor: STRIP.mark,
    boxShadow: `0 0 0 1px ${STRIP.mark}`,
  },
});
