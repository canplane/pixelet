import * as React from "react";
import { Button } from "@/shared/ui/theme/cozy/button.tsx";
import { cn } from "@/shared/lib/utils/cn.ts";

interface CircularButtonProps extends React.ComponentProps<typeof Button> {
  isActive?: boolean;
  /** Strip the floating-card chrome (background/border/shadow) for use inside a shared control tray. */
  bare?: boolean;
}

export function CircularButton({
  children,
  className,
  isActive,
  bare,
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
        "relative w-14 h-14 rounded-full transition-all duration-500 active:scale-95 group",
        !bare && "border-2 border-white/90 shadow-[0_4px_14px_rgba(200,160,100,0.22)]",
        isActive && "ring-2 ring-amber-400/60 bg-amber-50/80",
        disabled && "opacity-40 grayscale-[0.5] cursor-not-allowed pointer-events-none shadow-none",
        className,
      )}
      {...props}
    >
      <div className={cn("relative flex items-center justify-center w-full h-full transition-colors", disabled ? "text-[#5B4636]/30" : "text-inherit")}>
        {children}
      </div>
    </Button>
  );
}
