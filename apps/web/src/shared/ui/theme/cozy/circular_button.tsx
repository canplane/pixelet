import * as React from "react";
import { Button } from "@/shared/ui/theme/cozy/button.tsx";
import { cn } from "@/shared/lib/utils/cn.ts";

interface CircularButtonProps extends React.ComponentProps<typeof Button> {
  isActive?: boolean;
  /** Strip the floating-card chrome (background/border/shadow) for use inside a shared control tray. */
  bare?: boolean;
  /**
   * Flat, blurred dark chrome (bg-neutral-900/80 + backdrop-blur, white
   * icon) instead of this theme's default warm-cream card — the map's
   * floating controls (compass, GPS/DevicePoseControl) are moving toward
   * this dark language (see palette_bar.tsx) while everything else using
   * this shared button keeps the light cozy default untouched.
   */
  dark?: boolean;
}

export function CircularButton({
  children,
  className,
  isActive,
  bare,
  dark,
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
        !bare && !dark && "border-2 border-white/90 shadow-[0_4px_14px_rgba(200,160,100,0.22)]",
        // explicit shadow-none, not just omitting the utility — otherwise
        // twMerge leaves the default Button variant's cream shadow as the
        // only shadow-* utility in the merged list, leaking through even
        // though `dark` never asked for it
        dark && "backdrop-blur-md border border-white/15 shadow-none text-white/80",
        // active-state bg and base `dark` bg used to be two separate cn()
        // entries, each its own !bg-* class — twMerge doesn't reliably dedupe
        // Tailwind v4's leading-`!` classes against each other, so both could
        // reach the browser and whichever wins depends on Tailwind's
        // generated stylesheet order, not this class list — silently
        // dropping the GPS button's active tint once `dark` was introduced.
        // fixed by computing one background class per state instead of
        // stacking two. Amber (not blue) matches the active convention used
        // elsewhere (scale_bar.tsx, dev_panel's toggle badge).
        dark ? (isActive ? "!bg-amber-500/20 ring-2 ring-amber-400/60" : "!bg-neutral-900/50") : (isActive ? "bg-amber-50/80 ring-2 ring-amber-400/60" : ""),
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
