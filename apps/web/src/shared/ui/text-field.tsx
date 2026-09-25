// @pxl/web/shared/ui/text-field.tsx

/**
 * A text input that carries the two browser behaviors nobody remembers
 * until they are bitten by them.
 *
 * **iOS zooms the page** when a focused input's computed font-size is under
 * 16px, and there is no way to opt out of that which does not also disable
 * pinch-zoom for the whole document (`maximum-scale=1`, a real accessibility
 * loss, which is why index.html's viewport meta deliberately does not carry
 * it). The only remedy is to be 16px on the devices that do it.
 *
 * That is a two-token rule — `text-base md:text-sm` — and it lived as those
 * two tokens inside one component's class string, discovered once and then
 * not applied to the next input anyone wrote. The search bar zoomed for
 * exactly that reason. It is appended *after* the caller's className so
 * tailwind-merge resolves it in this component's favor: a caller cannot
 * reintroduce the bug by passing `text-sm`, which is the whole point. A field
 * that genuinely wants a different size can set one in `style`.
 *
 * **An IME's Enter is not a submit.** Confirming a Korean candidate syllable
 * fires `keydown` with `key === "Enter"`, so a bare Enter handler sends the
 * message halfway through composing a word. `onSubmit` here is Enter minus
 * that, which is the only Enter a text field ever actually means.
 *
 * Both are the same kind of rule: something true of every text input, that a
 * caller can only get right by having been told. This is where being told
 * happens.
 */

import * as React from "react";

import { cn } from "@/shared/lib/util/cn.ts";

type TextFieldProps = Omit<React.ComponentPropsWithoutRef<"input">, "onSubmit"> & {
  /** Enter, with an IME composition's own Enter excluded. */
  onSubmit?: () => void;
};

export const TextField = React.forwardRef<HTMLInputElement, TextFieldProps>(
  function TextField({ className, type = "text", onSubmit, onKeyDown, ...props }, ref) {
    return (
      <input
        ref={ref}
        type={type}
        {...props}
        onKeyDown={(event) => {
          onKeyDown?.(event);
          if (onSubmit && event.key === "Enter" && !event.nativeEvent.isComposing) onSubmit();
        }}
        className={cn(className, "text-base md:text-sm")}
      />
    );
  },
);
