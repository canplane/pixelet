// @pxl/client/ui/drawer/grid.tsx

import { useContext, useRef, useState } from "react";
import {
  type LayoutChangeEvent,
  Pressable,
  StyleSheet,
  View,
} from "react-native";

import type { f64 } from "@xpute/kit/abi/word.ts";
import { selectionRing } from "@/ui/asset/swatch.tsx";
import { chrome, color, hairline } from "@/ui/component/theme.ts";
import { NamingContext } from "./name-tip.tsx";

// A cell is the area a press lands on, and it is at least one touch target
// (`chrome.target`) wide. The item drawn in it is smaller than the cell.
const GRID_GAP = 4;
const LIT_SCALE = 1.05;
const LIT_INK = 2;

/**
 * An item is `ITEM_SLOPE * cell + ITEM_BASE` wide. At that size a lit item,
 * scaled by LIT_SCALE and ringed 1 + LIT_INK pixels wide on each side, reaches
 * exactly halfway into the gap, so two lit neighbours never touch.
 */
const ITEM_SLOPE = 1 / LIT_SCALE;
const ITEM_BASE = (GRID_GAP - 2 * (1 + LIT_INK)) / LIT_SCALE;
export function itemOf(cell: f64): f64 {
  return ITEM_SLOPE * cell + ITEM_BASE;
}

function columnsOf(width: f64): f64 {
  return Math.max(
    1,
    Math.floor((width + GRID_GAP) / (chrome.target + GRID_GAP)),
  );
}

/**
 * The cells are sized so that the outermost items' edges line up with the
 * section's edges, as the content of the other tabs does. For that, the grid
 * reaches past the section on each side by the margin an item keeps inside its
 * cell.
 */
export function Grid({
  head,
  children,
}: {
  head: React.ReactNode;
  children: (cell: f64) => React.ReactNode;
}) {
  const [width, setWidth] = useState(0);
  const columns = columnsOf(width);
  const cell =
    width > 0
      ? (width - (columns - 1) * GRID_GAP - ITEM_BASE) /
        (columns - 1 + ITEM_SLOPE)
      : chrome.target;
  return (
    <View
      style={styles.section}
      onLayout={(e: LayoutChangeEvent) => setWidth(e.nativeEvent.layout.width)}
    >
      {head}
      <View
        style={[styles.grid, { marginHorizontal: -(cell - itemOf(cell)) / 2 }]}
      >
        {children(cell)}
      </View>
    </View>
  );
}

export function Item({
  label,
  active,
  cell,
  onPress,
  children,
}: {
  label: string;
  active: boolean;
  cell: f64;
  onPress: () => void;
  children: React.ReactNode;
}) {
  const naming = useContext(NamingContext);
  const ref = useRef<View>(null);
  return (
    <Pressable
      ref={ref}
      onPress={onPress}
      onPressIn={() => ref.current && naming?.show(label, ref.current)}
      onPressOut={() => naming?.hide()}
      accessibilityRole="button"
      accessibilityLabel={label}
      aria-selected={active}
      style={[styles.cell, { width: cell, height: cell }]}
    >
      {children}
    </Pressable>
  );
}

export const mark = StyleSheet.create({
  off: { boxShadow: `0 0 0 1px ${hairline}` },
  on: {
    boxShadow: selectionRing(color.ground, color.black, LIT_INK),
    transform: [{ scale: LIT_SCALE }],
  },
});

const styles = StyleSheet.create({
  section: { gap: 8 },
  grid: { flexDirection: "row", flexWrap: "wrap", gap: GRID_GAP },
  cell: { alignItems: "center", justifyContent: "center" },
});
