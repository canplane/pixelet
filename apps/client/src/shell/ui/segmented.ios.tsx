// @pxl/client/shell/ui/segmented.ios.tsx

/** UIKit's segmented control, as every touched control here is; a segment is a title or an image, never both. */

import { requireNativeView } from "expo";
import type { ComponentType } from "react";
import type { NativeSyntheticEvent, ViewProps } from "react-native";

import { PANEL_CONTROL } from "@/shell/ui/panel.tsx";
import type { SegmentedProps } from "@/shell/ui/segmented.tsx";

// Everything the default has, but what this file draws itself.
export * from "./segmented.tsx";

interface SegmentedControlProps extends ViewProps {
  labels: string[];
  selectedIndex: number;
  onPick: (event: NativeSyntheticEvent<{ index: number }>) => void;
}

const SegmentedControl: ComponentType<SegmentedControlProps> = requireNativeView("PxlPlatform", "SegmentedControl");

export function Segmented<T extends string | number>({ segments, value, onChange }: SegmentedProps<T>) {
  return (
    <SegmentedControl
      labels={segments.map((s) => s.label)}
      selectedIndex={segments.findIndex((s) => s.value === value)}
      onPick={(e) => onChange(segments[e.nativeEvent.index].value)}
      style={{ height: PANEL_CONTROL, alignSelf: "stretch" }}
    />
  );
}
