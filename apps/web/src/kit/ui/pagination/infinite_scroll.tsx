// @pxl/web/shared/ui/pagination/infinite_scroll.tsx

import { type ReactNode, useCallback, useEffect, useRef, useState } from "react";

/** Default intersection threshold for infinite scroll */
export const INTERSECTION_OBSERVER_THRESHOLD: number = 1.0; // A range of 0.1 to 0.5 also works well.

/**
 * Props for InfiniteScrollLayout
 */
type InfiniteScrollProps<T> = {
  /** Full list of items */
  items: T[];

  /** Number of items to load per batch */
  batchSize: number;

  /** Optional behavior overrides */
  options?: {
    /** Reset visible items when `items` changes (default: false) */
    resetOnItemsChange?: boolean;

    /** IntersectionObserver threshold (default: 1.0) */
    intersectionObserverThreshold?: number;
  };

  /** Render function receiving currently visible items */
  children: (visibleItems: T[]) => ReactNode;
};

/**
 * Generic infinite scroll layout using IntersectionObserver
 */
export function InfiniteScroll<T>(props: InfiniteScrollProps<T>) {
  const { children, items, batchSize, options = {} } = props;
  const {
    resetOnItemsChange = false,
    intersectionObserverThreshold = INTERSECTION_OBSERVER_THRESHOLD,
  } = options;

  // number of items currently visible
  const [displayCount, setDisplayCount] = useState<number>(batchSize);
  useEffect(() => {
    if (resetOnItemsChange) {
      setDisplayCount(batchSize);
    }
  }, [items]);

  // sentinel element observed by IntersectionObserver
  const observerTarget = useRef<HTMLDivElement>(null);

  const onIntersect: IntersectionObserverCallback = useCallback(
    (entries) => {
      if (entries[0].isIntersecting && displayCount < items.length) {
        setDisplayCount((prev) => Math.min(prev + batchSize, items.length));
      }
    },
    [displayCount, items.length],
  );

  useEffect(() => {
    const observer = new IntersectionObserver(onIntersect, {
      root: null,
      rootMargin: "0px",
      threshold: intersectionObserverThreshold,
    });
    if (observerTarget.current) observer.observe(observerTarget.current);

    return () => {
      if (observerTarget.current) observer.unobserve(observerTarget.current);
    };
  }, [onIntersect]);

  const visibleItems: T[] = items.slice(0, displayCount);

  return (
    <>
      {children(visibleItems)}

      {/* empty div for IntersectionObserver to watch */}
      <div ref={observerTarget} className="h-px w-full" />
    </>
  );
}
