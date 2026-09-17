// @pxl/web/shared/hook/use_timer.ts

import { useEffect, useRef } from "react";

/**
 * Runs a callback once after a specified delay.
 *
 * This hook guarantees that the latest version of the callback
 * is executed, even if it changes between renders.
 *
 * @param callback - Function to execute after the delay
 * @param delay - Delay in milliseconds (default: 0)
 */
export function useTimeout(callback: () => void, delay: number = 0): void {
  const savedCallback = useRef<() => void>(callback);

  /**
   * Keep the latest callback reference.
   * This avoids stale closures without re-registering the timer.
   */
  useEffect(() => {
    savedCallback.current = callback;
  }, [callback]);

  useEffect(() => {
    const id = setTimeout(() => savedCallback.current(), delay);
    return () => clearTimeout(id);
  }, [delay]);
}

/**
 * Repeatedly runs a callback at a specified interval.
 *
 * The callback reference is kept up to date to ensure
 * the most recent logic is executed on each tick.
 *
 * @param callback - Function to execute on each interval
 * @param delay - Interval duration in milliseconds (default: 0)
 */
export function useInterval(callback: () => void, delay: number = 0): void {
  const savedCallback = useRef<() => void>(callback);

  /**
   * Keep the latest callback reference.
   * This prevents stale closures inside the interval handler.
   */
  useEffect(() => {
    savedCallback.current = callback;
  }, [callback]);

  useEffect(() => {
    const tick = () => savedCallback.current();
    const id = setInterval(tick, delay);
    return () => clearInterval(id);
  }, [delay]);
}
