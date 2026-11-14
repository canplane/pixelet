// @pxl/client/ui/feature/drawer/grid.tsx

import { useContext, useRef, useState } from "react";
import {
  type LayoutChangeEvent,
  Pressable,
  StyleSheet,
  View,
} from "react-native";

import type { f64 } from "@xpute/kit/abi/word.ts";
import { selection_ring } from "@/ui/shared/asset/swatch.tsx";
import { CHROME, COLOR, HAIRLINE } from "@/ui/shared/component/theme.ts";
import { NamingContext } from "./name-tip.tsx";

// A cell is the area a press lands on, and it is at least one touch target
// (`CHROME.target`) wide. The item drawn in it is smaller than the cell.
const GRID_GAP = 4;
const LIT_SCALE = 1.05;
const LIT_INK = 2;

/**
 * An item is `ITEM_SLOPE * cell + ITEM_BASE` wide. At that size a lit item,
 * scaled by LIT_SCALE and ringed 1 + LIT_INK pixels wide on each side, reaches
 * exactly halfway into the gap, so two lit neighbors never touch.
 */
const ITEM_SLOPE = 1 / LIT_SCALE;
const ITEM_BASE = (GRID_GAP - 2 * (1 + LIT_INK)) / LIT_SCALE;
export function item_of(cell: f64): f64 {
  return ITEM_SLOPE * cell + ITEM_BASE;
}

function column_count(width: f64): f64 {
  return Math.max(
    1,
    Math.floor((width + GRID_GAP) / (CHROME.target + GRID_GAP)),
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
  const [width, set_width] = useState(0);
  const columns = column_count(width);
  const cell =
    width > 0
      ? (width - (columns - 1) * GRID_GAP - ITEM_BASE) /
        (columns - 1 + ITEM_SLOPE)
      : CHROME.target;
  return (
    <View
      style={STYLE.section}
      onLayout={(e: LayoutChangeEvent) => set_width(e.nativeEvent.layout.width)}
    >
      {head}
      <View
        style={[STYLE.grid, { marginHorizontal: -(cell - item_of(cell)) / 2 }]}
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
      style={[STYLE.cell, { width: cell, height: cell }]}
    >
      {children}
    </Pressable>
  );
}

export const MARK = StyleSheet.create({
  off: { boxShadow: `0 0 0 1px ${HAIRLINE}` },
  on: {
    boxShadow: selection_ring(COLOR.ground, COLOR.black, LIT_INK),
    transform: [{ scale: LIT_SCALE }],
  },
});

const STYLE = StyleSheet.create({
  section: { gap: 8 },
  grid: { flexDirection: "row", flexWrap: "wrap", gap: GRID_GAP },
  cell: { alignItems: "center", justifyContent: "center" },
});
