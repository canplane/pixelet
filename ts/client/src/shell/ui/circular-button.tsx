// @pxl/client/shell/ui/circular-button.tsx

import { Pressable, StyleSheet } from "react-native";

import type { f64 } from "@xpute/kit/abi/word.ts";
import { Backing, SYSTEM_BACKING } from "@/shell/ui/backing.tsx";
import type { GlyphName } from "@/shell/ui/icon.tsx";
import { LABEL } from "@/shell/ui/palette.ts";
import { BackingPlace } from "@/shell/ui/backing.tsx";
import { Glyph } from "@/shell/ui/glyph.tsx";
import { Motion } from "@/shell/ui/motion/motion.tsx";
import { SNAP } from "@/shell/ui/motion/transition.ts";
import { chrome, color } from "@/shell/ui/theme.ts";

interface CircularButtonProps {
  glyph: GlyphName;
  size?: f64;
  place?: BackingPlace;
  onPress: () => void;
  label: string;
}

export function CircularButton({ glyph, size = chrome.control, place = BackingPlace.OverMap, onPress, label }: CircularButtonProps) {
  const round = { width: size, height: size, borderRadius: size / 2 };
  return (
    <Pressable onPress={onPress} accessibilityRole="button" accessibilityLabel={label} style={round}>
      {({ pressed }) => (
        <Motion to={{ scale: pressed && !SYSTEM_BACKING ? 0.95 : 1 }} transition={SNAP} style={[styles.press, { borderRadius: size / 2 }]}>
          <Backing
            fill={place === BackingPlace.OnSheet ? color.faint : color.ground}
            place={place}
            interactive
            style={[styles.face, { borderRadius: size / 2 }]}
          >
            <Glyph glyph={glyph} size={chrome.glyph} color={LABEL.primary} />
          </Backing>
        </Motion>
      )}
    </Pressable>
  );
}

const styles = StyleSheet.create({
  press: { flex: 1 },
  face: { flex: 1, alignItems: "center", justifyContent: "center" },
});
