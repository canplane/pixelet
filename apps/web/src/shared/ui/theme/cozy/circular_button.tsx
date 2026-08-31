import * as React from "react";
import { Button } from "@/shared/ui/theme/cozy/button.tsx";
import { cn } from "@/shared/lib/utils/cn.ts";

interface CircularButtonProps extends React.ComponentProps<typeof Button> {
  isActive?: boolean;
  bare?: boolean;
  /**
   * No backdrop-blur, deliberately. This sits over a WebGL canvas that
   * repaints every frame, so a backdrop-filter's re-blur cost is paid
   * continuously rather than once — the leading suspect behind a Galaxy S23
   * janking on a build an iPhone 13 mini ran fine (see AGENTS.md's canvas
   * overlay rules).
   */
  dark?: boolean;
  shadow?: boolean;
}

export function CircularButton({
  children,
  className,
  isActive,
  bare,
  dark,
  shadow = true,
  variant,
  size = "icon",
  disabled,
  ...props
}: CircularButtonProps) {
  return (
    <Button
      variant={variant ?? (bare ? "ghost" : "default")}
      size={size}
      disabled={disabled}
      className={cn(
        "relative w-14 h-14 rounded-full cursor-pointer transition-all duration-500 active:scale-95 group",
        !bare && !dark && (shadow ? "shadow-[0_6px_20px_rgba(0,0,0,0.25)]" : "shadow-none"),
        dark && cn(
          "border border-white/15 text-white/80",
          shadow ? "shadow-[0_4px_14px_rgba(0,0,0,0.35)]" : "shadow-none",
        ),
        // Computed as one class per state, not stacked separately: twMerge
        // doesn't reliably dedupe Tailwind v4's leading-`!` classes against
        // each other, so two separate `!bg-*` entries could both reach the
        // browser and silently lose the active tint depending on stylesheet
        // order.
        dark ? (isActive ? "!bg-amber-500/25 ring-2 ring-amber-400/60" : "!bg-neutral-900/60") : (isActive ? "bg-amber-50 ring-2 ring-amber-400/60" : ""),
        disabled && "opacity-40 grayscale-[0.5] cursor-not-allowed pointer-events-none shadow-none",
        className,
      )}
      {...props}
    >
      <div
        className={cn(
          "relative flex items-center justify-center w-full h-full transition-colors",
          disabled ? (dark ? "text-white/20" : "text-[#5B4636]/30") : "text-inherit",
        )}
      >
        {children}
      </div>
    </Button>
  );
}
