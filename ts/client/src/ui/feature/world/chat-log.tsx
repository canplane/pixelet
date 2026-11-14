// @pxl/client/ui/feature/world/chat-log.tsx

import { useTranslation } from "react-i18next";
import { StyleSheet, Text, View } from "react-native";

import { useChatLines } from "@/ui/feature/world/dock/store.ts";
import { corner_card } from "@/ui/shared/component/corner-card.ts";
import {
  alpha,
  color,
  text_style,
  weight,
} from "@/ui/shared/component/theme.ts";
import { textEdge } from "@/ui/shared/component/typeface.ts";

export function ChatLog() {
  const { t } = useTranslation();
  const chat_lines = useChatLines();

  if (chat_lines.length === 0) return null;

  return (
    <View style={styles.lines}>
      {chat_lines.map((line, i) => (
        <View
          key={line.id}
          style={[
            styles.line,
            {
              opacity: 0.4 + 0.6 * ((i + 1) / chat_lines.length),
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
  lines: { ...corner_card, alignItems: "flex-start", gap: 4 },
  line: {
    flexDirection: "row",
    alignItems: "baseline",
    gap: 8,
    maxWidth: "100%",
  },
  text: {
    ...text_style.caption,
    fontWeight: weight.semibold,
    ...textEdge(alpha(color.black, 0.9)),
  },
  self: { flexShrink: 0, color: color.self },
  said: { flexShrink: 1, color: color.white },
});
