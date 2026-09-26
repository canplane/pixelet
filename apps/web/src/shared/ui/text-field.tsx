// @pxl/web/shared/ui/text-field.tsx

/**
 * A text input that carries the behaviors nobody remembers until they are
 * bitten by them.
 *
 * **iOS Safari zooms the page** when a focused input's font size is under
 * 16px, and there is no way to opt out of that which does not also disable
 * pinch-zoom for the whole document (`maximum-scale=1`, a real accessibility
 * loss, which is why index.html's viewport meta deliberately does not carry
 * it). The only remedy is to be 16px, so the size is applied *after* the
 * caller's style: a caller cannot reintroduce the bug by passing a smaller
 * one, which is the whole point.
 *
 * **An IME's Enter is not a submit.** Confirming a Korean candidate syllable
 * fires Enter; `onSubmitEditing` already leaves that one out on the web
 * (react-native-web reads the key event's composition), and a device's own
 * keyboard never sends it.
 *
 * **A submit keeps the field.** A single-line input blurs itself on submit by
 * default, which closes a phone's keyboard between two messages.
 */

import type { Ref } from "react";
import { StyleSheet, TextInput, type TextInputProps } from "react-native";

import { textStyle } from "@/shared/ui/theme.ts";

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
      // A device reads `submitBehavior`; react-native-web still reads only
      // the older `blurOnSubmit`, so both say the same thing.
      submitBehavior="submit"
      blurOnSubmit={false}
      style={[styles.field, style, styles.size]}
    />
  );
}

const styles = StyleSheet.create({
  field: textStyle.input,
  size: { fontSize: textStyle.input.fontSize },
});
