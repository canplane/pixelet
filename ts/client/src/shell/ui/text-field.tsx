// @pxl/client/shell/ui/text-field.tsx

/**
 * The font size is applied after the caller's style: iOS Safari zooms into a
 * focused field under 16px, and `maximum-scale=1` would kill pinch-zoom. A
 * submit keeps focus, so a phone's keyboard stays up between messages.
 * RN-web reads `caretColor` as CSS; a device reads `selectionColor`.
 */

import type { Ref } from "react";
import {
  Platform,
  StyleSheet,
  TextInput,
  type TextInputProps,
  type TextStyle,
} from "react-native";

import { color, textScale, textStyle } from "@/shell/ui/theme.ts";

/** An iPhone's field draws its own clear button; a page and Android draw theirs. */
export const SYSTEM_CLEAR: boolean = Platform.OS === "ios";

type TextFieldProps = Omit<TextInputProps, "onSubmitEditing"> & {
  ref?: Ref<TextInput>;
  /** Enter, with an IME composition's own Enter excluded. */
  onSubmit?: () => void;
};

export function TextField({ style, onSubmit, ...props }: TextFieldProps) {
  return (
    <TextInput
      {...props}
      onSubmitEditing={onSubmit && (() => onSubmit())}
      // RN-web still reads only the older `blurOnSubmit`.
      submitBehavior="submit"
      blurOnSubmit={false}
      maxFontSizeMultiplier={textScale.chrome}
      selectionColor={color.chosen}
      cursorColor={color.chosen}
      style={[styles.field, style, styles.size, styles.caret]}
    />
  );
}

const styles = StyleSheet.create({
  field: textStyle.input,
  size: { fontSize: textStyle.input.fontSize },
  caret: { caretColor: color.chosen } as TextStyle,
});
