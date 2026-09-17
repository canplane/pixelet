import * as React from "react";
import { Button } from "@/kit/ui/shadcn/button.tsx";
import { cn } from "@/kit/lib/util/cn.ts";

interface CircularButtonProps extends React.ComponentProps<typeof Button> {
  isActive?: boolean;
  shadow?: boolean;
}

export function CircularButton({
  children,
  className,
  isActive,
  shadow = true,
  variant,
  size = "icon",
  disabled,
  ...props
}: CircularButtonProps) {
  return (
    <Button
      variant={variant ?? "default"}
      size={size}
      disabled={disabled}
      className={cn(
        "relative w-14 h-14 rounded-full cursor-pointer transition-all duration-500 active:scale-95 group",
        shadow ? "shadow-[0_6px_20px_rgba(0,0,0,0.25)]" : "shadow-none",
        isActive ? "bg-amber-50 ring-2 ring-amber-400/60" : "",
        disabled && "opacity-40 grayscale-[0.5] cursor-not-allowed pointer-events-none shadow-none",
        className,
      )}
      {...props}
    >
      <div
        className={cn(
          "relative flex items-center justify-center w-full h-full transition-colors",
          disabled ? "text-[#5B4636]/30" : "text-inherit",
        )}
      >
        {children}
      </div>
    </Button>
  );
}
