// @pxl/client/ui/feature/world/top/top.tsx

import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Pressable, StyleSheet, View } from "react-native";
import { Defs, LinearGradient, Mask, Rect, Stop, Svg } from "react-native-svg";

import { openModal } from "@/ui/shared/modal.ts";
import { Presence } from "@/ui/shared/component/motion/presence.tsx";
import { LEAVE, SETTLE } from "@/ui/shared/component/motion/transition.ts";
import { HaloIcon } from "./halo.tsx";
import { CHROME, COLOR, LAYER } from "@/ui/shared/component/theme.ts";
import { CurrentAddress } from "./current-address.tsx";
import { SensorIcons } from "./sensor-icon.tsx";
import { MapScaleBar } from "./scale-bar.tsx";
import { CompassBar } from "./compass-bar.tsx";
import { LocationStack } from "./location-stack.tsx";
import { ViewButtons } from "./view-button.tsx";
import { BusyBlock, BusyDim } from "@/ui/feature/world/busy-dim.tsx";
import { useChromeTop } from "@/ui/feature/world/chrome.ts";
import { CircularButton } from "@/ui/shared/component/circular-button.tsx";
import { useWindowSize } from "@/service/platform/viewport.ts";
import {
  type DragMode,
  dragMode,
  watchDragMode,
  watchZoom,
} from "@/service/core/input.ts";

export function TopOverlays() {
  const { t } = useTranslation();
  const top = useChromeTop();

  const { width } = useWindowSize();
  const scrimDepth = top + CHROME.control + SCRIM_TAIL;
  const from = (width - Math.min(width, SCRIM_WIDTH)) / 2 - SCRIM_FADE;
  const ramp = SCRIM_FADE / (width - 2 * from);

  return (
    <>
      <View style={[STYLE.scrim, { height: scrimDepth }]}>
        <Svg width="100%" height="100%">
          <Defs>
            <LinearGradient id="scrim-down" x1="0" y1="0" x2="0" y2="1">
              {SCRIM_EASE.map(([at, k]) => (
                <Stop
                  key={at}
                  offset={at}
                  stopColor={COLOR.black}
                  stopOpacity={SCRIM_OPACITY * (1 - k)}
                />
              ))}
            </LinearGradient>
            <LinearGradient
              id="scrim-across"
              gradientUnits="userSpaceOnUse"
              x1={from}
              y1={0}
              x2={width - from}
              y2={0}
            >
              {[
                ...SCRIM_EASE.map(([at, k]) => [ramp * at, k] as const),
                ...[...SCRIM_EASE]
                  .reverse()
                  .map(([at, k]) => [1 - ramp * at, k] as const),
              ].map(([at, k], i) => (
                <Stop
                  key={i}
                  offset={at}
                  stopColor={COLOR.white}
                  stopOpacity={k}
                />
              ))}
            </LinearGradient>
            <Mask id="scrim-mask">
              <Rect width="100%" height="100%" fill="url(#scrim-across)" />
            </Mask>
          </Defs>
          <Rect
            width="100%"
            height="100%"
            fill="url(#scrim-down)"
            mask="url(#scrim-mask)"
          />
        </Svg>
      </View>

      <BusyDim style={[STYLE.left, { top }]}>
        <CircularButton
          glyph="line.3.horizontal"
          onPress={() => openModal("settings")}
          label={t("settings.trigger_aria_label")}
        />
      </BusyDim>

      <BusyBlock style={[STYLE.center, { top }]}>
        <SearchStrip />
      </BusyBlock>

      <BusyDim style={[STYLE.right, { top }]}>
        <LocationStack />
        <ViewButtons />
      </BusyDim>
    </>
  );
}

function SearchStrip() {
  const { t } = useTranslation();
  const shown = useGaugesShown();
  return (
    <Pressable
      onPress={() => openModal("search")}
      accessibilityRole="button"
      accessibilityLabel={t("world.search.trigger_aria_label")}
      style={STYLE.strip}
    >
      <Presence
        show={!shown}
        from={{ opacity: 0 }}
        exit={{ opacity: 0 }}
        transition={SETTLE}
        exitTransition={LEAVE}
        style={STYLE.title}
      >
        <View style={STYLE.search}>
          <HaloIcon
            glyph="magnifyingglass"
            size={12}
            weight="semibold"
            color={COLOR.white}
          />
          <CurrentAddress />
        </View>
        <SensorIcons />
      </Presence>
      <View style={STYLE.gauges}>
        <CompassBar show={shown} />
        <MapScaleBar show={shown} />
      </View>
    </Pressable>
  );
}

/** How long the gauges stay once the hand turning or zooming the view stops. */
const LINGER_MS = 500;

/** Whether the pointers turn or zoom the view: a turning drag or a pinch. */
function turnsOrZooms(mode: DragMode): boolean {
  return mode === "turn" || mode === "pinch";
}

/** Whether the gauges, the compass bar and the scale, show: while a pointer
 * turns or pinches the view, and for `LINGER_MS` after it lets go or after a
 * zoom by the wheel or a trackpad's pinch. Only the user's own input shows
 * them, so the view turning with the device does not. It is set where the
 * input is told, so the frame a hand lets go in already lingers. */
function useGaugesShown(): boolean {
  const [shown, setShown] = useState(() => turnsOrZooms(dragMode()));
  useEffect(() => {
    let timer: ReturnType<typeof setTimeout> | undefined;
    let holding = turnsOrZooms(dragMode());
    const linger = () => {
      setShown(true);
      clearTimeout(timer);
      timer = setTimeout(() => setShown(false), LINGER_MS);
    };
    const offMode = watchDragMode(() => {
      const now = turnsOrZooms(dragMode());
      if (now) {
        clearTimeout(timer);
        setShown(true);
      } else if (holding) {
        linger();
      }
      holding = now;
    });
    const offZoom = watchZoom(() => {
      if (!holding) linger();
    });
    return () => {
      offMode();
      offZoom();
      clearTimeout(timer);
    };
  }, []);
  return shown;
}

/** A dark scrim along the top, so that white chrome reads over a bright world
 * without a blur. It is flat across `SCRIM_WIDTH` and then fades out along a
 * smoothstep, because the end of a straight fade reads as an edge. */
const SCRIM_OPACITY = 0.45;
const SCRIM_WIDTH = 560;
const SCRIM_FADE = 240;
const SCRIM_TAIL = 32;
const SCRIM_EASE = Array.from({ length: 9 }, (_, i) => {
  const t = i / 8;
  return [t, t * t * (3 - 2 * t)] as const;
});

const STYLE = StyleSheet.create({
  scrim: {
    position: "absolute",
    top: 0,
    left: 0,
    right: 0,
    zIndex: LAYER.scrim,
    pointerEvents: "none",
  },
  left: {
    position: "absolute",
    left: CHROME.inset,
    zIndex: LAYER.chrome,
    alignItems: "flex-start",
    gap: 12,
  },
  center: {
    position: "absolute",
    left: 0,
    right: 0,
    paddingHorizontal: CHROME.inset + CHROME.control + CHROME.controlGap,
    height: CHROME.control,
    zIndex: LAYER.chrome,
  },
  right: {
    position: "absolute",
    right: CHROME.inset,
    zIndex: LAYER.chrome,
    alignItems: "center",
    gap: CHROME.controlGap,
  },
  // The HUD is the strip: the title (the address over the sensors) or, while
  // the view turns or zooms, the gauges in its place. The whole strip takes
  // the press, whichever shows. It is as high as the buttons' row: the title
  // stands in its middle, and the gauges hang from its top.
  strip: { flex: 1, alignItems: "center", justifyContent: "center" },
  title: { maxWidth: "100%", alignItems: "center", gap: 8 },
  // The row is bounded, because the web lays out a row that does not wrap at
  // its content's width, and a long address would run over the buttons.
  search: {
    flexDirection: "row",
    alignItems: "center",
    gap: 8,
    minWidth: 0,
    maxWidth: "100%",
    flexShrink: 1,
  },
  gauges: {
    position: "absolute",
    top: 0,
    left: 0,
    right: 0,
    alignItems: "center",
    gap: 12,
    pointerEvents: "none",
  },
});
