// @pxl/client/shell/drawer/grid.tsx

import { useContext, useRef, useState } from "react";
import {
  type LayoutChangeEvent,
  Pressable,
  StyleSheet,
  View,
} from "react-native";

import type { f64 } from "@xpute/kit/abi/word.ts";
import { heldRing } from "@/shell/catalog/asset/swatch.tsx";
import { chrome, color, hairline } from "@/shell/ui/theme.ts";
import { NamingContext } from "./name-tip.tsx";

// A press lands on 44 at least: a cell is the press, a target at least, and the item one sees is fitted inside it.
const GRID_GAP = 4;
const LIT_SCALE = 1.05;
const LIT_INK = 2;

/** Lit, an item reaches no further than halfway into the gap: a cell's `ITEM_SLOPE` plus `ITEM_BASE`. */
const ITEM_SLOPE = 1 / LIT_SCALE;
const ITEM_BASE = (GRID_GAP - 2 * (1 + LIT_INK)) / LIT_SCALE;
export function itemOf(cell: f64): f64 {
  return ITEM_SLOPE * cell + ITEM_BASE;
}

/** How many cells a target wide, with their gaps, fit `width`; one at least. */
function columnsOf(width: f64): f64 {
  return Math.max(
    1,
    Math.floor((width + GRID_GAP) / (chrome.target + GRID_GAP)),
  );
}

/**
 * As many columns as fit a target each, the cells sized so the outer items'
 * edges meet the row's, as the other tabs' content does: the cells overhang by
 * the margin an item keeps inside its cell.
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

/** Pictures only: names under every item read as a list. A press names it above the finger. */
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

/** An item's outline, and lit, the ring and the scale `ITEM_BASE` leaves room for. */
export const mark = StyleSheet.create({
  off: { boxShadow: `0 0 0 1px ${hairline}` },
  on: {
    boxShadow: heldRing(color.ground, color.black, LIT_INK),
    transform: [{ scale: LIT_SCALE }],
  },
});

const styles = StyleSheet.create({
  section: { gap: 8 },
  grid: { flexDirection: "row", flexWrap: "wrap", gap: GRID_GAP },
  cell: { alignItems: "center", justifyContent: "center" },
});
