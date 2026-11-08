// @pxl/client/ui/component/segmented.ios.tsx

/** An iPhone's segmented control is UIKit's, as is every control a person
 * touches on an iPhone. A UIKit segment shows a title or an image, never
 * both. */

import { requireNativeView } from "expo";
import type { ComponentType } from "react";
import type { NativeSyntheticEvent, ViewProps } from "react-native";

import { PANEL_CONTROL } from "@/ui/component/panel.tsx";
import type { SegmentedProps } from "@/ui/component/segmented.tsx";

// Every export of the default file, except `Segmented`: an export declared in
// this file takes the place of a star export with the same name.
export * from "./segmented.tsx";

interface SegmentedControlProps extends ViewProps {
  labels: string[];
  selectedIndex: number;
  onPick: (event: NativeSyntheticEvent<{ index: number }>) => void;
}

const SegmentedControl: ComponentType<SegmentedControlProps> =
  requireNativeView("PxlPlatform", "SegmentedControl");

export function Segmented<T extends string | number>({
  segments,
  value,
  onChange,
}: SegmentedProps<T>) {
  return (
    <SegmentedControl
      labels={segments.map((s) => s.label)}
      selectedIndex={segments.findIndex((s) => s.value === value)}
      onPick={(e) => onChange(segments[e.nativeEvent.index].value)}
      style={{ height: PANEL_CONTROL, alignSelf: "stretch" }}
    />
  );
}
