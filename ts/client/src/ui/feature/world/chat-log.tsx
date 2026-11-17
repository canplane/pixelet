// @pxl/client/ui/feature/world/chat-log.tsx

import { useTranslation } from "react-i18next";
import { StyleSheet, Text, View } from "react-native";

import { useChatLines } from "@/ui/feature/world/dock/store.ts";
import { CORNER_CARD } from "@/ui/shared/component/corner-card.ts";
import {
  alpha,
  COLOR,
  TEXT_STYLE,
  WEIGHT,
} from "@/ui/shared/component/theme.ts";
import { textHalo } from "@/ui/shared/component/typeface.ts";

export function ChatLog() {
  const { t } = useTranslation();
  const chatLines = useChatLines();

  if (chatLines.length === 0) return null;

  return (
    <View style={STYLE.lines}>
      {chatLines.map((line, i) => (
        <View
          key={line.id}
          style={[
            STYLE.line,
            {
              opacity: 0.4 + 0.6 * ((i + 1) / chatLines.length),
            },
          ]}
        >
          <Text style={[STYLE.text, STYLE.self]}>
            {t("world.chat.self_label")}
          </Text>
          <Text style={[STYLE.text, STYLE.said]}>{line.text}</Text>
        </View>
      ))}
    </View>
  );
}

const STYLE = StyleSheet.create({
  lines: { ...CORNER_CARD, alignItems: "flex-start", gap: 4 },
  line: {
    flexDirection: "row",
    alignItems: "baseline",
    gap: 8,
    maxWidth: "100%",
  },
  text: {
    ...TEXT_STYLE.caption,
    fontWeight: WEIGHT.semibold,
    ...textHalo(alpha(COLOR.black, 0.9)),
  },
  self: { flexShrink: 0, color: COLOR.self },
  said: { flexShrink: 1, color: COLOR.white },
});
