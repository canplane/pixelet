// @pxl/client/ui/component/section-title.tsx

import type { ReactNode } from "react";
import { type StyleProp, StyleSheet, Text, type TextStyle } from "react-native";

import { LABEL } from "@/ui/component/palette.ts";
import { sectionTitle } from "@/ui/component/theme.ts";

export function SectionTitle({
  style,
  children,
}: {
  style?: StyleProp<TextStyle>;
  children: ReactNode;
}) {
  return (
    <Text accessibilityRole="header" style={[styles.title, style]}>
      {children}
    </Text>
  );
}

const styles = StyleSheet.create({
  title: { ...sectionTitle, color: LABEL.secondary },
});
