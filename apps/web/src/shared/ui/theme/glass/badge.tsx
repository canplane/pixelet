import * as React from "react";
import { Slot } from "@radix-ui/react-slot";
import { cva, type VariantProps } from "class-variance-authority";

import { cn } from "@/shared/lib/utils/cn.ts";

const badgeVariants = cva(
  // wider letter-spacing, smaller (text-[10px]) font for a refined feel
  "inline-flex items-center justify-center rounded-full border px-2 py-0.5 text-[10px] font-semibold tracking-wider uppercase transition-colors",
  {
    variants: {
      variant: {
        default: "border-transparent bg-white text-black",
        glass: "bg-white/10 backdrop-blur-md border-white/20 text-white/90",
        outline: "border-white/20 text-white/70",
      },
    },
    defaultVariants: {
      variant: "default",
    },
  },
);

function Badge({
  className,
  variant,
  asChild = false,
  ...props
}:
  & React.ComponentProps<"span">
  & VariantProps<typeof badgeVariants>
  & { asChild?: boolean }) {
  const Comp = asChild ? Slot : "span";

  return (
    <Comp
      data-slot="badge"
      className={cn(badgeVariants({ variant }), className)}
      {...props}
    />
  );
}

export { Badge, badgeVariants };
