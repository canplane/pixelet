import * as React from "react";
import { Slot } from "@radix-ui/react-slot";
import { cva, type VariantProps } from "class-variance-authority";

import { cn } from "@/shared/lib/utils/cn.ts";

const buttonVariants = cva(
  // tight letter-spacing, medium weight, soft corner radius
  "inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-2xl text-sm font-medium transition-all duration-200 active:scale-[0.98] disabled:pointer-events-none disabled:opacity-40 [&_svg]:size-4 outline-none focus-visible:ring-1 focus-visible:ring-white/30 tracking-tight",
  {
    variants: {
      variant: {
        // opaque but smooth white/black
        default: "bg-white text-black shadow-[0_4px_12px_rgba(0,0,0,0.1)] hover:bg-white/90",

        // glass: the core of this theme — clear, translucent, with a visible edge
        glass: "bg-white/10 backdrop-blur-md border border-white/20 border-t-white/40 \
           shadow-[0_8px_32px_rgba(0,0,0,0.05),inset_0_0_0_1px_rgba(255,255,255,0.05)] \
           text-white hover:bg-white/15 hover:border-white/30",

        outline: "border border-white/20 bg-transparent text-white hover:bg-white/5",

        ghost: "text-white/80 hover:bg-white/10 hover:text-white",

        secondary: "bg-white/5 text-white/90 hover:bg-white/10 backdrop-blur-sm",
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
