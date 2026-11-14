// @pxl/client/ui/feature/drawer/character/picked.tsx

import { useState } from "react";
import { type StyleProp, StyleSheet, View, type ViewStyle } from "react-native";

import type { u32 } from "@xpute/kit/abi/word.ts";
import { Spinner } from "@/ui/shared/component/icon.tsx";
import { LABEL } from "@/ui/shared/component/palette.ts";
import { surface_driver } from "@/service/core/surface.ts";
import type { Character } from "./character.ts";
import { CHROME, COLOR } from "@/ui/shared/component/theme.ts";
import { CharacterSurface } from "@/service/platform/surface.tsx";
import { useCore } from "@/ui/shared/watch.ts";
import * as scene from "@/service/core/scene.ts";

const SURFACE = surface_driver();
const BACKDROP = parseInt(COLOR.backdrop.slice(1), 16);

interface PickedCharacterProps {
  character: Character;
  style?: StyleProp<ViewStyle>;
}

export function PickedCharacter(props: PickedCharacterProps) {
  const { character, style } = props;
  const [surface, set_surface] = useState<u32>(0);
  const drawn = useCore(
    (s) => surface !== 0 && s.character_surfaces.includes(surface),
  );

  return (
    <View style={style}>
      {
        // The surface stays hidden until the core has drawn this character,
        // because until then its canvas holds the last character's frame.
      }
      <View style={[StyleSheet.absoluteFill, !drawn && styles.hidden]}>
        <CharacterSurface
          surface={SURFACE}
          openKey={`${character.group}:${character.number}`}
          onOpen={(surface) => {
            set_surface(surface);
            scene.draw_character(surface, character.group, BACKDROP);
            return () => {};
          }}
        />
      </View>
      {!drawn && (
        <View style={styles.loading}>
          <Spinner size={CHROME.glyph} color={LABEL.secondary} />
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
