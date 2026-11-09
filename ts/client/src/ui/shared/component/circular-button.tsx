// @pxl/client/ui/shared/component/circular-button.tsx

import { Pressable, StyleSheet } from "react-native";

import type { f64 } from "@xpute/kit/abi/word.ts";
import { Backing, SYSTEM_BACKING } from "@/ui/shared/component/backing.tsx";
import type { GlyphName } from "@/ui/shared/component/icon.tsx";
import { LABEL } from "@/ui/shared/component/palette.ts";
import { BackingPlace } from "@/ui/shared/component/backing.tsx";
import { Glyph } from "@/ui/shared/component/glyph.tsx";
import { Motion } from "@/ui/shared/component/motion/motion.tsx";
import { SNAP } from "@/ui/shared/component/motion/transition.ts";
import { chrome, color } from "@/ui/shared/component/theme.ts";

interface CircularButtonProps {
  glyph: GlyphName;
  size?: f64;
  place?: BackingPlace;
  onPress: () => void;
  label: string;
}

export function CircularButton({
  glyph,
  size = chrome.control,
  place = BackingPlace.OverMap,
  onPress,
  label,
}: CircularButtonProps) {
  const round = { width: size, height: size, borderRadius: size / 2 };
  return (
    <Pressable
      onPress={onPress}
      accessibilityRole="button"
      accessibilityLabel={label}
      style={round}
    >
      {({ pressed }) => (
        <Motion
          to={{ scale: pressed && !SYSTEM_BACKING ? 0.95 : 1 }}
          transition={SNAP}
          style={[styles.press, { borderRadius: size / 2 }]}
        >
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
