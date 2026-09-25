// @pxl/web/feature/map/ui/overlay/dummy-chat-text.tsx

import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Pressable, StyleSheet, Text, useWindowDimensions, View } from "react-native";

import { useUiStore } from "@/app/store/ui-store.ts";
import { alpha, color, font, textEdge } from "@/shared/ui/theme.ts";

/** The lines said so far, fading with age; a pointer over them brings them
 * all back up to be read. */
export function DummyChatText() {
  const { t } = useTranslation();
  const { width } = useWindowDimensions();
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
  line: { flexDirection: "row", alignItems: "baseline", gap: 6, maxWidth: "100%" },
  // A dark edge so white words read over any ground.
  text: {
    fontFamily: font.family,
    fontSize: 12,
    fontWeight: "500",
    ...textEdge(alpha(color.black, 0.9)),
  },
  self: { flexShrink: 0, color: "#ff8904" },
  said: { flexShrink: 1, color: color.white },
});
