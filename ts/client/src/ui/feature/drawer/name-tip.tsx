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
import { color, textStyle } from "@/ui/shared/component/theme.ts";

const TIP_POINT = 5;
/** The width of the box the tip is centered in, over the item. It must hold
 * the longest name. */
const TIP_SPAN = 240;
/**
 * The pill and its point are filled solid and faded together as one layer.
 * Faded apart, they would overlap where a device rounds their pixels together
 * and show a darker line there.
 */
const TIP_FILL = color.black;
const TIP_OPACITY = 0.72;
const TIP_PAD_Y = 5;
const TIP_RADIUS = (textStyle.notice.lineHeight + 2 * TIP_PAD_Y) / 2;

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

function NameTip({ named, sheetHeight }: { named: Named; sheetHeight: f64 }) {
  return (
    <View
      pointerEvents="none"
      style={[
        styles.tipAnchor,
        {
          left: named.x - TIP_SPAN / 2,
          bottom: sheetHeight - named.y,
        },
      ]}
    >
      <View style={styles.tipShape}>
        <View
          needsOffscreenAlphaCompositing
          style={[StyleSheet.absoluteFill, styles.tipFill]}
        >
          <View style={styles.tipPill} />
          <View style={styles.tipPoint} />
        </View>
        <Text numberOfLines={1} style={styles.tip}>
          {named.label}
        </Text>
      </View>
    </View>
  );
}

export function NameTipLayer({ children }: { children: ReactNode }) {
  const sheet = useRef<View>(null);
  const [sheetHeight, setSheetHeight] = useState(0);
  const [named, setNamed] = useState<Named | null>(null);
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
              setNamed({ label, x: x - sx + w / 2, y: y - sy });
            }
          }),
        );
      },
      hide: () => {
        press.current++;
        setNamed(null);
      },
    }),
    [],
  );

  return (
    <View
      ref={sheet}
      style={styles.sheet}
      onLayout={(e: LayoutChangeEvent) =>
        setSheetHeight(e.nativeEvent.layout.height)
      }
    >
      <NamingContext.Provider value={naming}>{children}</NamingContext.Provider>
      {named && <NameTip named={named} sheetHeight={sheetHeight} />}
    </View>
  );
}

const styles = StyleSheet.create({
  sheet: { flex: 1 },
  tipAnchor: {
    position: "absolute",
    width: TIP_SPAN,
    alignItems: "center",
    paddingBottom: 2,
  },
  tipShape: { alignItems: "center", paddingBottom: TIP_POINT },
  tipFill: { opacity: TIP_OPACITY, alignItems: "center" },
  tipPill: {
    alignSelf: "stretch",
    flex: 1,
    backgroundColor: TIP_FILL,
    borderRadius: TIP_RADIUS,
  },
  tip: {
    ...textStyle.notice,
    color: color.white,
    paddingHorizontal: 12,
    paddingVertical: TIP_PAD_Y,
  },
  tipPoint: {
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
