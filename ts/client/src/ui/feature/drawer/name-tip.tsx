// @pxl/client/ui/feature/drawer/name-tip.tsx

import {
  createContext,
  type ReactNode,
  useMemo,
  useRef,
  useState,
} from "react";
import { type LayoutChangeEvent, StyleSheet, Text, View } from "react-native";

import type { f64 } from "@xpute/kit/abi/word.ts";
import { COLOR, TEXT_STYLE } from "@/ui/shared/component/theme.ts";

const TIP_POINT = 5;
/** The width of the box the tip is centered in, over the item. It must hold
 * the longest name. */
const TIP_SPAN = 240;
/**
 * The pill and its point are filled solid and faded together as one layer.
 * Faded apart, they would overlap where a device rounds their pixels together
 * and show a darker line there.
 */
const TIP_FILL = COLOR.black;
const TIP_OPACITY = 0.72;
const TIP_PAD_Y = 5;
const TIP_RADIUS = (TEXT_STYLE.notice.lineHeight + 2 * TIP_PAD_Y) / 2;

/** The name is drawn in the sheet's top layer, over its bar: a name drawn
 * inside the scrolling content could not reach over the bar. */
interface Naming {
  show: (label: string, item: View) => void;
  hide: () => void;
}
export const NamingContext = createContext<Naming | null>(null);

interface Named {
  label: string;
  x: f64;
  y: f64;
}

function NameTip({ named, sheetHeightPx }: { named: Named; sheetHeightPx: f64 }) {
  return (
    <View
      pointerEvents="none"
      style={[
        STYLE.tip_anchor,
        {
          left: named.x - TIP_SPAN / 2,
          bottom: sheetHeightPx - named.y,
        },
      ]}
    >
      <View style={STYLE.tip_shape}>
        <View
          needsOffscreenAlphaCompositing
          style={[StyleSheet.absoluteFill, STYLE.tip_fill]}
        >
          <View style={STYLE.tip_pill} />
          <View style={STYLE.tip_point} />
        </View>
        <Text numberOfLines={1} style={STYLE.tip}>
          {named.label}
        </Text>
      </View>
    </View>
  );
}

export function NameTipLayer({ children }: { children: ReactNode }) {
  const sheet = useRef<View>(null);
  const [sheet_height, set_sheet_height] = useState(0);
  const [named, set_named] = useState<Named | null>(null);
  // The sheet and the item are both measured in the window. On a device,
  // measureLayout against the sheet leaves out how far the content scrolled.
  // Each press takes a new number, so a measurement that answers after its
  // press was released finds a newer number and shows nothing.
  const press = useRef(0);
  const naming = useMemo<Naming>(
    () => ({
      show: (label, item) => {
        const at = ++press.current;
        sheet.current?.measureInWindow((sx, sy) =>
          item.measureInWindow((x, y, w) => {
            if (press.current === at) {
              set_named({ label, x: x - sx + w / 2, y: y - sy });
            }
          }),
        );
      },
      hide: () => {
        press.current++;
        set_named(null);
      },
    }),
    [],
  );

  return (
    <View
      ref={sheet}
      style={STYLE.sheet}
      onLayout={(e: LayoutChangeEvent) =>
        set_sheet_height(e.nativeEvent.layout.height)
      }
    >
      <NamingContext.Provider value={naming}>{children}</NamingContext.Provider>
      {named && <NameTip named={named} sheetHeightPx={sheet_height} />}
    </View>
  );
}

const STYLE = StyleSheet.create({
  sheet: { flex: 1 },
  tip_anchor: {
    position: "absolute",
    width: TIP_SPAN,
    alignItems: "center",
    paddingBottom: 2,
  },
  tip_shape: { alignItems: "center", paddingBottom: TIP_POINT },
  tip_fill: { opacity: TIP_OPACITY, alignItems: "center" },
  tip_pill: {
    alignSelf: "stretch",
    flex: 1,
    backgroundColor: TIP_FILL,
    borderRadius: TIP_RADIUS,
  },
  tip: {
    ...TEXT_STYLE.notice,
    color: COLOR.white,
    paddingHorizontal: 12,
    paddingVertical: TIP_PAD_Y,
  },
  tip_point: {
    // The point overlaps the pill by a pixel, so that no rounding opens a gap
    // between them. They are faded as one layer, so the overlap does not show.
    marginTop: -1,
    width: 0,
    height: 0,
    borderLeftWidth: TIP_POINT,
    borderRightWidth: TIP_POINT,
    borderTopWidth: TIP_POINT,
    borderLeftColor: "transparent",
    borderRightColor: "transparent",
    borderTopColor: TIP_FILL,
  },
});
