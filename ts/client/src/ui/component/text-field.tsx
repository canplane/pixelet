// @pxl/client/ui/component/text-field.tsx

/**
 * A text field whose font size overrides the caller's style. iOS Safari zooms
 * in on a focused field whose text is smaller than 16px, and the page cannot
 * forbid that zoom with `maximum-scale=1` without also forbidding pinch-zoom.
 * A submit keeps the focus, so that a phone's keyboard stays up between
 * messages.
 */

import type { Ref } from "react";
import {
  StyleSheet,
  TextInput,
  type TextInputProps,
  type TextStyle,
} from "react-native";

import { color, textScale, textStyle } from "@/ui/component/theme.ts";

/** Whether the platform's field draws its own clear button: on a page and on
 * Android it does not, and the caller draws one; an iPhone's does
 * (text-field.ios.tsx). */
export const SYSTEM_CLEAR: boolean = false;

type TextFieldProps = Omit<TextInputProps, "onSubmitEditing"> & {
  ref?: Ref<TextInput>;
  /** Called on Enter, except the Enter that ends an IME composition. */
  onSubmit?: () => void;
};

export function TextField({ style, onSubmit, ...props }: TextFieldProps) {
  return (
    <TextInput
      {...props}
      onSubmitEditing={onSubmit && (() => onSubmit())}
      // react-native-web reads only the older `blurOnSubmit`.
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
  // react-native-web writes `caretColor` out as CSS. A device reads
  // `selectionColor` and `cursorColor` instead.
  caret: { caretColor: color.chosen } as TextStyle,
});
