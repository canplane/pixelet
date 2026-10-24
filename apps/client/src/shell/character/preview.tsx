// @pxl/client/shell/character/preview.tsx

/** The variant picker's display case: the kernel draws the character spinning in place. */

import { useState } from "react";
import { type StyleProp, StyleSheet, View, type ViewStyle } from "react-native";

import type { u32 } from "@xpute/kit/abi/word.ts";
import { Spinner } from "@/shell/ui/icon.tsx";
import { LABEL } from "@/shell/ui/palette.ts";
import { CaseSurface } from "@/bracket.tsx";
import { Command, encode_preview_close, encode_preview_open } from "@/vmm/abi/cmd.spec.ts";
import { kernel_command, kernel_room, useKernel } from "@/vmm/shell.ts";
import type { CharacterVariant } from "@/shell/character/variant.ts";
import { chrome } from "@/shell/ui/theme.ts";

interface CharacterPreviewProps {
  variant: CharacterVariant;
  style?: StyleProp<ViewStyle>;
}

export function CharacterPreview(props: CharacterPreviewProps) {
  const { variant, style } = props;
  const [surface, setSurface] = useState<u32>(0);
  const drawn = useKernel((s) => surface !== 0 && s.previews.includes(surface));

  return (
    <View style={style}>
      {
        // Hidden until drawn: the canvas still holds the last variant's frame.
      }
      <View style={[StyleSheet.absoluteFill, !drawn && styles.hidden]}>
        <CaseSurface
          openKey={`${variant.group}:${variant.number}`}
          onOpen={(surface) => {
            setSurface(surface);
            kernel_command(Command.PREVIEW_OPEN, encode_preview_open(kernel_room, surface, variant.group, variant.number));
            return () => kernel_command(Command.PREVIEW_CLOSE, encode_preview_close(kernel_room, surface));
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
  loading: { ...StyleSheet.absoluteFill, alignItems: "center", justifyContent: "center", pointerEvents: "none" },
});
