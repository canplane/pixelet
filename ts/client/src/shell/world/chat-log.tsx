// @pxl/client/shell/world/chat-log.tsx

import { useTranslation } from "react-i18next";
import { StyleSheet, Text, View } from "react-native";

import { useDockStore } from "@/shell/store/dock.ts";
import { cornerCard } from "@/shell/ui/corner-card.ts";
import { alpha, color, textStyle, weight } from "@/shell/ui/theme.ts";
import { textEdge } from "@/shell/ui/typeface.ts";

/** The lines said, oldest faintest. */
export function ChatLog() {
  const { t } = useTranslation();
  const chatLines = useDockStore((s) => s.chatLines);

  if (chatLines.length === 0) return null;

  return (
    <View style={styles.lines}>
      {chatLines.map((line, i) => (
        <View
          key={line.id}
          style={[
            styles.line,
            {
              opacity: 0.4 + 0.6 * ((i + 1) / chatLines.length),
            },
          ]}
        >
          <Text style={[styles.text, styles.self]}>
            {t("world.chat.self_label")}
          </Text>
          <Text style={[styles.text, styles.said]}>{line.text}</Text>
        </View>
      ))}
    </View>
  );
}

const styles = StyleSheet.create({
  lines: { ...cornerCard, alignItems: "flex-start", gap: 4 },
  line: {
    flexDirection: "row",
    alignItems: "baseline",
    gap: 8,
    maxWidth: "100%",
  },
  text: {
    ...textStyle.caption,
    fontWeight: weight.semibold,
    ...textEdge(alpha(color.black, 0.9)),
  },
  self: { flexShrink: 0, color: color.self },
  said: { flexShrink: 1, color: color.white },
});
