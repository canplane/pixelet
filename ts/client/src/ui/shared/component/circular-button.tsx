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
import { CHROME, COLOR } from "@/ui/shared/component/theme.ts";

interface CircularButtonProps {
  glyph: GlyphName;
  size?: f64;
  place?: BackingPlace;
  onPress: () => void;
  label: string;
  /** The button is not offered now: it takes no press and is drawn faint. */
  disabled?: boolean;
}

export function CircularButton({
  glyph,
  size = CHROME.control,
  place = BackingPlace.OVER_MAP,
  onPress,
  label,
  disabled = false,
}: CircularButtonProps) {
  const round = { width: size, height: size, borderRadius: size / 2 };
  return (
    <Pressable
      onPress={onPress}
      disabled={disabled}
      accessibilityRole="button"
      accessibilityLabel={label}
      accessibilityState={{ disabled }}
      style={round}
    >
      {({ pressed }) => (
        <Motion
          to={{ scale: pressed && !SYSTEM_BACKING ? 0.95 : 1 }}
          transition={SNAP}
          style={[STYLE.press, { borderRadius: size / 2 }]}
        >
          <Backing
            fill={place === BackingPlace.ON_SHEET ? COLOR.faint : COLOR.ground}
            place={place}
            interactive
            style={[STYLE.face, { borderRadius: size / 2 }]}
          >
            <Glyph
              glyph={glyph}
              size={CHROME.glyph}
              color={disabled ? LABEL.tertiary : LABEL.primary}
            />
          </Backing>
        </Motion>
      )}
    </Pressable>
  );
}

const STYLE = StyleSheet.create({
  press: { flex: 1 },
  face: { flex: 1, alignItems: "center", justifyContent: "center" },
});
