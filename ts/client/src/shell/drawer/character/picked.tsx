// @pxl/client/shell/drawer/character/picked.tsx

/** The character picked, on its surface: the kernel draws the companion's character as it is. */

import { useState } from "react";
import { type StyleProp, StyleSheet, View, type ViewStyle } from "react-native";

import type { u32 } from "@xpute/kit/abi/word.ts";
import { Spinner } from "@/shell/ui/icon.tsx";
import { LABEL } from "@/shell/ui/palette.ts";
import { surface_driver } from "@/port/surface.ts";
import type { Character } from "./character.ts";
import { chrome, color } from "@/shell/ui/theme.ts";
import { CharacterSurface } from "@/service/surface.tsx";
import { useKernel } from "@/shell/store/watch.ts";
import * as scene from "@/port/scene.ts";

const SURFACE = surface_driver();
const BACKDROP = parseInt(color.backdrop.slice(1), 16);

interface PickedCharacterProps {
  character: Character;
  style?: StyleProp<ViewStyle>;
}

export function PickedCharacter(props: PickedCharacterProps) {
  const { character, style } = props;
  const [surface, setSurface] = useState<u32>(0);
  const drawn = useKernel(
    (s) => surface !== 0 && s.character_surfaces.includes(surface),
  );

  return (
    <View style={style}>
      {
        // Hidden until drawn: the canvas still holds the last character's frame.
      }
      <View style={[StyleSheet.absoluteFill, !drawn && styles.hidden]}>
        <CharacterSurface
          surface={SURFACE}
          openKey={`${character.group}:${character.number}`}
          onOpen={(surface) => {
            setSurface(surface);
            scene.draw_character(surface, character.group, BACKDROP);
            return () => {};
          }}
        />
      </View>
      {!drawn && (
        <View style={styles.loading}>
          <Spinner size={chrome.glyph} color={LABEL.secondary} />
        </View>
      )}
    </View>
  );
}

const styles = StyleSheet.create({
  hidden: { opacity: 0 },
  loading: {
    ...StyleSheet.absoluteFill,
    alignItems: "center",
    justifyContent: "center",
    pointerEvents: "none",
  },
});
