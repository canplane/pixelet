// @pxl/client/shared/ui/toggle-row.ts

import type { ReactNode } from "react";
import type { StyleProp, ViewStyle } from "react-native";

export interface ToggleRowProps {
  value: boolean;
  onValueChange: (value: boolean) => void;
  style?: StyleProp<ViewStyle>;
  children: ReactNode;
}
