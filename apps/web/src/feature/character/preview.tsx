// @pxl/web/feature/character/preview.tsx

/**
 * Small display case for the variant picker — one character playing its idle
 * clip while slowly spinning in place, so a pick can actually be seen before
 * committing to it. The surface is handed to the kernel as a case; the actor,
 * its model, its spin and the drawing are the kernel's
 * (crates/pxl/web/kernel/src/layer/character/preview.rs,
 * crates/pxl/web/render/src/gpu/renderer.rs).
 */

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
