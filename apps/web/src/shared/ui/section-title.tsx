// @pxl/web/shared/ui/section-title.tsx

import type { ReactNode } from "react";
import { type StyleProp, StyleSheet, Text, type TextStyle } from "react-native";

import { LABEL } from "@platform/palette.ts";
import { sectionTitle } from "@/shared/ui/theme.ts";

export function SectionTitle({ style, children }: { style?: StyleProp<TextStyle>; children: ReactNode }) {
  return <Text accessibilityRole="header" style={[styles.title, style]}>{children}</Text>;
}

const styles = StyleSheet.create({
  title: { ...sectionTitle, color: LABEL.secondary },
});
