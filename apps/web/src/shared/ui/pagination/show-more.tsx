// @pxl/web/shared/ui/pagination/show-more.tsx

import type { u32 } from "@xpute/core/abi/word.ts";
import { type ReactNode, useEffect, useState } from "react";

/**
 * Props for ShowMoreLayout
 */
type ShowMoreProps<T> = {
  /** Full list of items */
  items: T[];

  /** Number of items to reveal per click */
  batchSize: u32;

  /** Optional behavior overrides */
  options?: {
    /** Reset visible items when `items` changes (default: false) */
    resetOnItemsChange?: boolean;
  };

  /** Render function receiving currently visible items */
  children: (visibleItems: T[]) => ReactNode;

  /**
   * Render prop that receives an onClick handler
   * and returns a fully bound trigger node (e.g. button)
   */
  trigger: (onClick: () => void) => ReactNode;
};

/**
 * Progressive "Show More" layout (manual pagination)
 */
export function ShowMoreLayout<T>(props: ShowMoreProps<T>) {
  const { children, trigger, items, batchSize, options = {} } = props;
  const { resetOnItemsChange = false } = options;

  // number of items currently visible
  const [displayCount, setDisplayCount] = useState<u32>(batchSize);
  useEffect(() => {
    if (resetOnItemsChange) {
      setDisplayCount(batchSize);
    }
  }, [items]);

  // on click "Show more" button
  const handleShowMore = (): void => {
    setDisplayCount((prev) => Math.min(prev + batchSize, items.length));
  };

  const visibleItems: T[] = items.slice(0, displayCount);

  return (
    <>
      {children(visibleItems)}

      {displayCount < items.length && trigger(handleShowMore)}
    </>
  );
}
