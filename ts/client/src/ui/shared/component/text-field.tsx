// @pxl/client/ui/shared/component/text-field.tsx

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
  type TextInputInstance,
  type TextInputProps,
  type TextStyle,
} from "react-native";

import {
  COLOR,
  TEXT_SCALE_CAP,
  TEXT_STYLE,
} from "@/ui/shared/component/theme.ts";

/** Whether the platform's field draws its own clear button: on the web and on
 * Android it does not, and the caller draws one; an iPhone's does
 * (text-field.ios.tsx). */
export const SYSTEM_CLEAR: boolean = false;

type TextFieldProps = Omit<TextInputProps, "onSubmitEditing"> & {
  ref?: Ref<TextInputInstance>;
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
      maxFontSizeMultiplier={TEXT_SCALE_CAP.chrome}
      selectionColor={COLOR.chosen}
      cursorColor={COLOR.chosen}
      style={[STYLE.field, style, STYLE.size, STYLE.caret]}
    />
  );
}

const STYLE = StyleSheet.create({
  field: TEXT_STYLE.input,
  size: { fontSize: TEXT_STYLE.input.fontSize },
  // react-native-web writes `caretColor` out as CSS. A native field reads
  // `selectionColor` and `cursorColor` instead.
  caret: { caretColor: COLOR.chosen } as TextStyle,
});
