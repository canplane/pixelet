// @pxl/web/feature/character/preview.tsx

/** The variant picker's display case: the kernel draws the character spinning in place. */

import { useState } from "react";
import { type StyleProp, StyleSheet, View, type ViewStyle } from "react-native";

import type { u32 } from "@xpute/core/abi/word.ts";
import { Spinner } from "@platform/icon.tsx";
import { LABEL } from "@platform/palette.ts";
import { CaseSurface } from "@platform/surface.tsx";
import { Command, encode_preview_close, encode_preview_open } from "@/kernel/abi/cmd.spec.ts";
import { kernel_command, useKernel } from "@/kernel/shell.ts";
import type { CharacterVariant } from "@/feature/character/variant.ts";
import { chrome } from "@/shared/ui/theme.ts";

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
      <View style={[styles.fill, !drawn && styles.hidden]}>
        <CaseSurface
          openKey={`${variant.group}:${variant.number}`}
          onOpen={(surface) => {
            setSurface(surface);
            kernel_command(Command.PREVIEW_OPEN, encode_preview_open({ surface, group: variant.group, number: variant.number }));
            return () => kernel_command(Command.PREVIEW_CLOSE, encode_preview_close({ surface }));
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
  fill: { position: "absolute", top: 0, bottom: 0, left: 0, right: 0 },
  hidden: { opacity: 0 },
  loading: { position: "absolute", top: 0, bottom: 0, left: 0, right: 0, alignItems: "center", justifyContent: "center", pointerEvents: "none" },
});
