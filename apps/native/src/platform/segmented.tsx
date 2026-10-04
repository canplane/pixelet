// @pxl/native/platform/segmented.tsx

/** UIKit's segmented control, as every touched control here is; a segment is a title or an image, never both. */

import { requireNativeView } from "expo";
import type { ComponentType } from "react";
import type { NativeSyntheticEvent, ViewProps } from "react-native";

import { PANEL_CONTROL } from "@/shared/ui/panel.tsx";

export interface Segment<T extends string | number> {
  value: T;
  label: string;
}

interface SegmentedControlProps extends ViewProps {
  labels: string[];
  selectedIndex: number;
  onPick: (event: NativeSyntheticEvent<{ index: number }>) => void;
}

const SegmentedControl: ComponentType<SegmentedControlProps> = requireNativeView("PxlPlatform", "SegmentedControl");

export function Segmented<T extends string | number>({ segments, value, onChange }: { segments: readonly Segment<T>[]; value: T; onChange: (value: T) => void }) {
  return (
    <SegmentedControl
      labels={segments.map((s) => s.label)}
      selectedIndex={segments.findIndex((s) => s.value === value)}
      onPick={(e) => onChange(segments[e.nativeEvent.index].value)}
      style={{ height: PANEL_CONTROL, alignSelf: "stretch" }}
    />
  );
}
