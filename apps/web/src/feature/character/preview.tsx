// @pxl/web/feature/character/preview.tsx

/** The variant picker's display case: the kernel draws the character spinning in place. */

import { type StyleProp, View, type ViewStyle } from "react-native";

import { CaseSurface } from "@platform/surface.tsx";
import { Command, encode_preview_close, encode_preview_open } from "@/kernel/abi/cmd.spec.ts";
import { kernel_command } from "@/kernel/shell.ts";
import type { CharacterVariant } from "@/feature/character/variant.ts";

interface CharacterPreviewProps {
  variant: CharacterVariant;
  style?: StyleProp<ViewStyle>;
}

export function CharacterPreview(props: CharacterPreviewProps) {
  const { variant, style } = props;

  return (
    <View style={style}>
      <CaseSurface
        openKey={`${variant.group}:${variant.number}`}
        onOpen={(surface) => {
          kernel_command(Command.PREVIEW_OPEN, encode_preview_open({ surface, group: variant.group, number: variant.number }));
          return () => kernel_command(Command.PREVIEW_CLOSE, encode_preview_close({ surface }));
        }}
      />
    </View>
  );
}
