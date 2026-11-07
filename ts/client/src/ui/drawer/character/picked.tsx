// @pxl/client/ui/drawer/character/picked.tsx

/** The character picked, on its surface: the core draws the companion's character as it is. */

import { useState } from "react";
import { type StyleProp, StyleSheet, View, type ViewStyle } from "react-native";

import type { u32 } from "@xpute/kit/abi/word.ts";
import { Spinner } from "@/ui/component/icon.tsx";
import { LABEL } from "@/ui/component/palette.ts";
import { surface_driver } from "@/port/surface.ts";
import type { Character } from "./character.ts";
import { chrome, color } from "@/ui/component/theme.ts";
import { CharacterSurface } from "@/service/surface.tsx";
import { useCore } from "@/ui/store/watch.ts";
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
  const drawn = useCore(
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
