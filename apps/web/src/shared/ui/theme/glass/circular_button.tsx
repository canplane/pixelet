import * as React from "react";
import { Button } from "@/shared/ui/theme/glass/button.tsx";
import { cn } from "@/shared/lib/utils/cn.ts";

interface CircularButtonProps extends React.ComponentProps<typeof Button> {
  isActive?: boolean;
}

export function CircularButton({
  children,
  className,
  isActive,
  variant = "glass",
  size = "icon",
  disabled,
  ...props
}: CircularButtonProps) {
  return (
    <Button
      variant={variant}
      size={size}
      disabled={disabled}
      className={cn(
        "relative w-14 h-14 rounded-full transition-all duration-500 active:scale-95 group",
        "border-t-white/40 border-white/5 shadow-2xl",
        isActive && "ring-2 ring-amber-400/60 bg-amber-500/10",
        disabled &&
          "opacity-40 grayscale-[0.5] cursor-not-allowed pointer-events-none shadow-none border-t-white/10",
        className,
      )}
      {...props}
    >
      <div
        className={cn(
          "absolute top-0 left-1/2 -translate-x-1/2 w-10 h-5 bg-white/8 rounded-full blur-xs pointer-events-none transition-opacity",
          disabled ? "opacity-0" : "opacity-100",
        )}
      />

      <div
        className={cn(
          "flex items-center justify-center w-full h-full transition-colors",
          disabled ? "text-white/20" : "text-inherit",
        )}
      >
        {children}
      </div>

      {!disabled && <div className="absolute bottom-1 left-1/2 -translate-x-1/2 w-6 h-2 bg-white/5 rounded-full blur-[3px] pointer-events-none" />}
    </Button>
  );
}
