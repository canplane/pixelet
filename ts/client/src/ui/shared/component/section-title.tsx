// @pxl/client/ui/shared/component/section-title.tsx

import type { ReactNode } from "react";
import { type StyleProp, StyleSheet, Text, type TextStyle } from "react-native";

import { LABEL } from "@/ui/shared/component/palette.ts";
import { SECTION_TITLE } from "@/ui/shared/component/theme.ts";

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
  title: { ...SECTION_TITLE, color: LABEL.secondary },
});
