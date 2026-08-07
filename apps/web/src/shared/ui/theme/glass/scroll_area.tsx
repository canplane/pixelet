"use client";

import * as React from "react";
import * as ScrollAreaPrimitive from "@radix-ui/react-scroll_area";

import { cn } from "@/shared/lib/utils/cn.ts";

function ScrollArea({
  className,
  children,
  ...props
}: React.ComponentProps<typeof ScrollAreaPrimitive.Root>) {
  return (
    <ScrollAreaPrimitive.Root
      data-slot="scroll_area"
      className={cn("relative", className)}
      {...props}
    >
      <ScrollAreaPrimitive.Viewport
        data-slot="scroll_area-viewport"
        className="focus-visible:ring-ring/50 size-full rounded-[inherit] transition-[color,box-shadow] outline-none focus-visible:ring-[3px] focus-visible:outline-1"
      >
        {children}
      </ScrollAreaPrimitive.Viewport>
      <ScrollBar />
      <ScrollAreaPrimitive.Corner />
    </ScrollAreaPrimitive.Root>
  );
}

function ScrollBar({
  className,
  orientation = "vertical",
  ...props
}: React.ComponentProps<typeof ScrollAreaPrimitive.ScrollAreaScrollbar>) {
  return (
    <ScrollAreaPrimitive.ScrollAreaScrollbar
      data-slot="scroll_area-scrollbar"
      orientation={orientation}
      className={cn(
        "flex touch-none p-0.5 transition-colors select-none", // slightly larger padding for a steadier feel
        orientation === "vertical" &&
          "h-full w-2 border-l border-l-transparent", // 2.5 -> 2, a touch slimmer
        orientation === "horizontal" &&
          "h-2 flex-col border-t border-t-transparent",
        className,
      )}
      {...props}
    >
      <ScrollAreaPrimitive.ScrollAreaThumb
        data-slot="scroll_area-thumb"
        // primary/30 or foreground/20 instead of bg-border, so the
        // scrollbar reads as a subtle overlay instead of hiding the background
        className="bg-primary/30 relative flex-1 rounded-full transition-colors hover:bg-primary/50 active:bg-primary/70"
      />
    </ScrollAreaPrimitive.ScrollAreaScrollbar>
  );
}

export { ScrollArea, ScrollBar };
