// @pxl/web/feature/map/ui/overlay/dummy-chat-text.tsx

import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Pressable, StyleSheet, Text, View } from "react-native";

import { useUiStore } from "@/app/store/ui-store.ts";
import { alpha, color, textEdge, textStyle, weight } from "@/shared/ui/theme.ts";
import { useWindowSize } from "@platform/viewport.ts";

export function DummyChatText() {
  const { t } = useTranslation();
  const { width } = useWindowSize();
  const chatLines = useUiStore((s) => s.chatLines);
  const [hovered, setHovered] = useState(false);

  if (chatLines.length === 0) return null;

  return (
    <Pressable onHoverIn={() => setHovered(true)} onHoverOut={() => setHovered(false)} style={[styles.lines, { maxWidth: Math.min(320, width - 32) }]}>
      {chatLines.map((line, i) => (
        <View key={line.id} style={[styles.line, { opacity: hovered ? 0.95 : 0.23 + 0.47 * ((i + 1) / chatLines.length) }]}>
          <Text style={[styles.text, styles.self]}>{t("map.chat.self_label")}</Text>
          <Text style={[styles.text, styles.said]}>{line.text}</Text>
        </View>
      ))}
    </Pressable>
  );
}

const styles = StyleSheet.create({
  lines: { alignItems: "flex-start", gap: 4 },
  line: { flexDirection: "row", alignItems: "baseline", gap: 8, maxWidth: "100%" },
  text: {
    ...textStyle.caption,
    fontWeight: weight.medium,
    ...textEdge(alpha(color.black, 0.9)),
  },
  self: { flexShrink: 0, color: color.self },
  said: { flexShrink: 1, color: color.white },
});
