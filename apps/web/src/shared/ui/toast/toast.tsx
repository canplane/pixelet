// @pxl/web/shared/ui/toast/toast.tsx

import { CheckCircle2, MapPinOff } from "lucide-react-native";
import { ActivityIndicator, StyleSheet, Text, View } from "react-native";

import { alpha, color, font } from "@/shared/ui/theme.ts";
import type { ToastSpec, ToastTone } from "./store.ts";

const SIDE = 32;
const GREEN = "#34C759";

function glyph(tone: ToastTone) {
  switch (tone) {
    case "loading":
      return <ActivityIndicator size="small" color={color.ink} />;
    case "error":
      return <MapPinOff size={16} color={color.red500} />;
    case "ready":
      return <CheckCircle2 size={16} strokeWidth={2.5} color={GREEN} />;
  }
}

/** One card of the deck: a glyph for its tone, what it says, and a dot. */
export function Toast({ message, tone }: ToastSpec) {
  const error = tone === "error";
  return (
    <View style={[styles.pill, error && styles.pillError]}>
      <View style={styles.slot}>{glyph(tone)}</View>
      <Text numberOfLines={1} style={styles.message}>{message}</Text>
      <View style={styles.slot}>
        <View style={[styles.dot, { backgroundColor: error ? color.red500 : tone === "ready" ? GREEN : alpha(GREEN, 0.3) }]} />
      </View>
    </View>
  );
}

const styles = StyleSheet.create({
  pill: {
    flexDirection: "row",
    alignItems: "center",
    height: SIDE,
    borderRadius: SIDE / 2,
    borderWidth: 2,
    borderColor: alpha(color.white, 0.9),
    backgroundColor: color.cream,
    boxShadow: "0 6px 20px rgba(0, 0, 0, 0.25)",
    overflow: "hidden",
  },
  pillError: { backgroundColor: "#fef2f2", borderColor: "#fecaca", boxShadow: "0 6px 20px rgba(220, 38, 38, 0.15)" },
  // A glyph's square, pulled over the pill's own border so a card that says
  // little still reads as a pill and not an oval.
  slot: { width: SIDE, height: SIDE, marginHorizontal: -2, alignItems: "center", justifyContent: "center" },
  message: { fontFamily: font.family, fontSize: 13, fontWeight: "600", letterSpacing: -0.3, color: color.ink, paddingHorizontal: 4 },
  dot: { width: 6, height: 6, borderRadius: 3 },
});
