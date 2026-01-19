// @pxl/web/shared/ui/pagination/show-more.tsx

import { useEffect, useState } from "react";

/**
 * Props for ShowMoreLayout
 */
type ShowMoreProps<T> = {
  /** Full list of items */
  items: T[];

  /** Number of items to reveal per click */
  batchSize: number;

  /** Optional behavior overrides */
  options?: {
    /** Reset visible items when `items` changes (default: false) */
    resetOnItemsChange?: boolean;
  };

  /** Render function receiving currently visible items */
  children: (visibleItems: T[]) => React.ReactNode;

  /**
   * Render prop that receives an onClick handler
   * and returns a fully bound trigger node (e.g. button)
   */
  trigger: (onClick: () => void) => React.ReactNode;
};

/**
 * Progressive "Show More" layout (manual pagination)
 */
export function ShowMoreLayout<T>(props: ShowMoreProps<T>) {
  const { children, trigger, items, batchSize, options = {} } = props;
  const { resetOnItemsChange = false } = options;

  // Number of items currently visible
  const [displayCount, setDisplayCount] = useState<number>(batchSize);
  useEffect(() => {
    if (resetOnItemsChange) {
      setDisplayCount(batchSize);
    }
  }, [items]);

  // On click "Show more" button
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
