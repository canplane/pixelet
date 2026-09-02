import * as React from "react";
import { Slot } from "@radix-ui/react-slot";
import { cva, type VariantProps } from "class-variance-authority";

import { cn } from "@/shared/lib/utils/cn.ts";

const buttonVariants = cva(
  "inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-2xl text-sm font-semibold transition-all duration-200 active:scale-[0.98] disabled:pointer-events-none disabled:opacity-40 [&_svg]:size-5 outline-none focus-visible:ring-2 focus-visible:ring-amber-400/50 tracking-tight",
  {
    variants: {
      variant: {
        default: "bg-[#FFFDF7] text-[#5B4636] shadow-[0_6px_20px_rgba(0,0,0,0.25)] hover:bg-[#FFF8EC]",

        accent: "bg-amber-400 text-white shadow-[0_4px_14px_rgba(217,119,6,0.35)] hover:bg-amber-500",

        outline: "border-2 border-[#F0E4CC] bg-transparent text-[#5B4636] hover:bg-[#FFF8EC]",

        ghost: "text-[#5B4636]/70 hover:bg-[#FFF3DE] hover:text-[#5B4636]",

        secondary: "bg-[#FFF3DE] text-[#5B4636] hover:bg-[#FCE8C6]",
      },
      size: {
        default: "h-10 px-5 py-2",
        sm: "h-8 px-3 text-xs",
        lg: "h-12 px-8 text-base",
        icon: "size-10",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  },
);

function Button({
  className,
  variant = "default",
  size = "default",
  asChild = false,
  ...props
}:
  & React.ComponentProps<"button">
  & VariantProps<typeof buttonVariants>
  & {
    asChild?: boolean;
  }) {
  const Comp = asChild ? Slot : "button";

  return (
    <Comp
      data-slot="button"
      data-variant={variant}
      data-size={size}
      className={cn(buttonVariants({ variant, size, className }))}
      {...props}
    />
  );
}

export { Button, buttonVariants };
